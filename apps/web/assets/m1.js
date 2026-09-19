(function () {
  "use strict";
  const decoderConfig = { codec: "vp8", codedWidth: 320, codedHeight: 180, optimizeForLatency: true };
  const encoderConfig = { codec: "vp8", width: 160, height: 90, bitrate: 500_000, framerate: 30, latencyMode: "realtime", hardwareAcceleration: "no-preference" };
  const MAX_DECODE_QUEUE = 4;
  const MAX_ENCODE_QUEUE = 4;
  const MAX_CALLBACK_QUEUE = 2;

  async function copyRgba(frame) {
    const rgba = new Uint8Array(frame.allocationSize({ format: "RGBA" }));
    const [layout] = await frame.copyTo(rgba, { format: "RGBA" });
    if (!layout) throw new Error("RGBA copy returned no plane layout");
    return { rgba, layout };
  }

  function rgbAt(rgba, layout, x, y) {
    const start = layout.offset + y * layout.stride + x * 4;
    return rgba.subarray(start, start + 3);
  }

  function parseIvf(bytes, manifest) {
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    if (String.fromCharCode(...bytes.slice(0, 4)) !== "DKIF") throw new Error("fixture is not IVF");
    const packets = [];
    let offset = view.getUint16(6, true);
    while (offset < bytes.byteLength) {
      const size = view.getUint32(offset, true);
      const lo = view.getUint32(offset + 4, true);
      const hi = view.getUint32(offset + 8, true);
      const timestamp = lo + hi * 0x1_0000_0000;
      offset += 12;
      packets.push({ timestamp, data: bytes.slice(offset, offset + size) });
      offset += size;
    }
    if (packets.length !== manifest.frame_count) throw new Error(`fixture packet count ${packets.length} != ${manifest.frame_count}`);
    return packets;
  }

  async function waitFor(predicate, label) {
    for (let i = 0; i < 20_000; i++) {
      if (predicate()) return;
      await new Promise(resolve => setTimeout(resolve, 0));
    }
    throw new Error(`timed out while draining ${label}`);
  }

  async function run(fixture, manifestText, processFrame, status, cancelled) {
    const manifest = JSON.parse(manifestText);
    const started = performance.now();
    const metrics = { decoded: 0, processed: 0, encoded: 0, verified: 0, explicitExternalCopies: 0, verificationReadbacks: 0, liveFrames: 0, peakLiveFrames: 0, peakDecoderQueue: 0, peakDecodedCallbacks: 0, peakEncoderQueue: 0, peakVerifierQueue: 0, peakVerifierCallbacks: 0 };
    const own = frame => { metrics.liveFrames++; metrics.peakLiveFrames = Math.max(metrics.peakLiveFrames, metrics.liveFrames); return frame; };
    const close = frame => { if (frame) { frame.close(); metrics.liveFrames--; } };
    let decoder, encoder, verifier;
    const decoded = [], encoded = [], verified = [];
    try {
      if (!globalThis.isSecureContext) throw new Error("secure context required (use localhost or HTTPS)");
      if (!navigator.gpu) throw new Error("WebGPU is unavailable");
      if (!globalThis.VideoDecoder || !globalThis.VideoEncoder || !globalThis.VideoFrame) throw new Error("WebCodecs VideoDecoder, VideoEncoder, or VideoFrame is unavailable");
      const [decoderSupport, encoderSupport] = await Promise.all([
        VideoDecoder.isConfigSupported(decoderConfig), VideoEncoder.isConfigSupported(encoderConfig)
      ]);
      if (!decoderSupport.supported) throw new Error(`VP8 decoder config unsupported: ${JSON.stringify(decoderConfig)}`);
      if (!encoderSupport.supported) throw new Error(`VP8 encoder config unsupported: ${JSON.stringify(encoderConfig)}`);
      // Canvas capture is exercised by the Rust frame bridge (HTML or OffscreenCanvas).
      status("Capabilities passed. Decoding deterministic VP8 fixture…");

      const packets = parseIvf(fixture, manifest);
      let decoderError;
      decoder = new VideoDecoder({ output: frame => { decoded.push(own(frame)); metrics.decoded++; metrics.peakDecodedCallbacks = Math.max(metrics.peakDecodedCallbacks, decoded.length); }, error: error => { decoderError = error; } });
      decoder.configure(decoderSupport.config);
      let encoderError;
      encoder = new VideoEncoder({ output: (chunk, metadata) => {
        const data = new Uint8Array(chunk.byteLength); chunk.copyTo(data);
        encoded.push({ type: chunk.type, timestamp: chunk.timestamp, duration: chunk.duration, data, decoderConfig: metadata?.decoderConfig });
        metrics.encoded++;
      }, error: error => { encoderError = error; } });
      encoder.configure(encoderSupport.config);
      const decoderFeed = (async () => {
        for (let i = 0; i < packets.length; i++) {
          while (decoder.decodeQueueSize >= MAX_DECODE_QUEUE || decoded.length >= MAX_CALLBACK_QUEUE) {
            if (cancelled()) throw new DOMException("cancelled", "AbortError");
            await new Promise(resolve => setTimeout(resolve, 0));
          }
          const timestamp = i * manifest.frame_duration_us;
          decoder.decode(new EncodedVideoChunk({ type: i === 0 ? "key" : "delta", timestamp, duration: manifest.frame_duration_us, data: packets[i].data }));
          metrics.peakDecoderQueue = Math.max(metrics.peakDecoderQueue, decoder.decodeQueueSize);
        }
        await decoder.flush();
      })();
      while (metrics.processed < manifest.frame_count) {
        if (cancelled()) throw new DOMException("cancelled", "AbortError");
        if (decoderError) throw decoderError;
        const frame = decoded.shift();
        if (!frame) { await new Promise(resolve => setTimeout(resolve, 0)); continue; }
        const timestamp = frame.timestamp;
        const duration = frame.duration ?? manifest.frame_duration_us;
        let output;
        try {
          output = own(await processFrame(frame, timestamp, duration));
          metrics.liveFrames--; // Rust closed the decoded input after GPU completion.
          metrics.explicitExternalCopies++;
          while (encoder.encodeQueueSize >= MAX_ENCODE_QUEUE) {
            if (cancelled()) throw new DOMException("cancelled", "AbortError");
            if (encoderError) throw encoderError;
            await new Promise(resolve => setTimeout(resolve, 0));
          }
          encoder.encode(output, { keyFrame: metrics.processed === 0 });
          metrics.peakEncoderQueue = Math.max(metrics.peakEncoderQueue, encoder.encodeQueueSize);
          close(output);
          metrics.processed++;
          status(`Processed ${metrics.processed}/${manifest.frame_count} frames (bounded GPU submissions).`);
        } catch (error) {
          if (output) close(output);
          else close(frame);
          throw error;
        }
      }
      await decoderFeed;
      await encoder.flush();
      if (encoderError) throw encoderError;
      decoder.close(); decoder = null;
      encoder.close(); encoder = null;

      status("Encoding drained. Re-decoding output and performing test-only pixel checks…");
      let verifierError;
      verifier = new VideoDecoder({ output: frame => { verified.push(own(frame)); metrics.peakVerifierCallbacks = Math.max(metrics.peakVerifierCallbacks, verified.length); }, error: error => { verifierError = error; } });
      const returnedConfig = encoded.find(packet => packet.decoderConfig)?.decoderConfig ?? { codec: "vp8", codedWidth: 160, codedHeight: 90 };
      const verifySupport = await VideoDecoder.isConfigSupported(returnedConfig);
      if (!verifySupport.supported) throw new Error(`returned encoder decoderConfig unsupported: ${JSON.stringify(returnedConfig)}`);
      verifier.configure(verifySupport.config);
      const verifierFeed = (async () => {
        for (const packet of encoded) {
          while (verifier.decodeQueueSize >= MAX_DECODE_QUEUE || verified.length >= MAX_CALLBACK_QUEUE) await new Promise(resolve => setTimeout(resolve, 0));
          verifier.decode(new EncodedVideoChunk(packet));
          metrics.peakVerifierQueue = Math.max(metrics.peakVerifierQueue, verifier.decodeQueueSize);
        }
        await verifier.flush();
      })();
      while (metrics.verified < manifest.frame_count) {
        if (verifierError) throw verifierError;
        const frame = verified.shift();
        if (!frame) { await new Promise(resolve => setTimeout(resolve, 0)); continue; }
        const i = metrics.verified;
        try {
          if (frame.codedWidth !== 160 || frame.codedHeight !== 90) throw new Error(`verification frame ${i} has ${frame.codedWidth}x${frame.codedHeight}`);
          if (Math.abs(frame.timestamp - i * manifest.frame_duration_us) > 1) throw new Error(`verification frame ${i} timestamp ${frame.timestamp} is incorrect`);
          if (frame.duration !== null && Math.abs(frame.duration - manifest.frame_duration_us) > 1) throw new Error(`verification frame ${i} duration ${frame.duration} is incorrect`);
          const { rgba, layout } = await copyRgba(frame);
          metrics.verificationReadbacks++;
          const topLeft = rgbAt(rgba, layout, 6, 6), topRight = rgbAt(rgba, layout, 153, 6), bottomLeft = rgbAt(rgba, layout, 6, 83);
          if (!(topLeft[0] > topLeft[1] * 1.25 && topRight[1] > topRight[0] * 1.15 && bottomLeft[2] > bottomLeft[0] * 1.25)) throw new Error(`orientation/color markers failed on frame ${i}: TL=${[...topLeft]} TR=${[...topRight]} BL=${[...bottomLeft]}`);
          metrics.verified++;
        } finally {
          close(frame);
        }
      }
      await verifierFeed;
      verifier.close(); verifier = null;
      if (metrics.verified !== manifest.frame_count) throw new Error(`verified ${metrics.verified}/${manifest.frame_count} frames`);
      if (metrics.liveFrames !== 0) throw new Error(`${metrics.liveFrames} application-owned frames remain live`);
      const elapsed = performance.now() - started;
      const summary = [
        `PASS: ${metrics.verified}/${manifest.frame_count} frames decoded → GPU-resized → encoded → re-decoded.`,
        `Output: 160×90 VP8; timestamps within ±1 µs; orientation and RGB markers passed.`,
        `Elapsed: ${elapsed.toFixed(1)} ms (includes verification; no throughput claim).`,
        `Copies: ${metrics.explicitExternalCopies} VideoFrame→texture API copies; 0 explicit CPU pixel readbacks in conversion; ${metrics.verificationReadbacks} test-only readbacks.`,
        `Peaks: live frames ${metrics.peakLiveFrames}; decoder submissions ${metrics.peakDecoderQueue}; decoded callbacks ${metrics.peakDecodedCallbacks}; encoder submissions ${metrics.peakEncoderQueue}; verifier submissions ${metrics.peakVerifierQueue}; verifier callbacks ${metrics.peakVerifierCallbacks}.`,
        `Cleanup: ${metrics.liveFrames} application-owned live frames. Browser-internal copies and codec hardware execution: unknown.`
      ].join("\n");
      return { summary, metrics, elapsedMs: elapsed, decoderConfig: returnedConfig };
    } catch (error) {
      for (const frame of decoded.splice(0)) close(frame);
      for (const frame of verified.splice(0)) close(frame);
      try { decoder?.reset(); decoder?.close(); } catch (_) {}
      try { encoder?.reset(); encoder?.close(); } catch (_) {}
      try { verifier?.reset(); verifier?.close(); } catch (_) {}
      const label = error?.name === "AbortError" ? "CANCELLED" : "FAILED";
      throw new Error(`${label}: ${error?.message ?? error}; cleanup live frames=${metrics.liveFrames}; processed=${metrics.processed}; encoded=${metrics.encoded}; verified=${metrics.verified}; peaks live=${metrics.peakLiveFrames},decode=${metrics.peakDecoderQueue},callbacks=${metrics.peakDecodedCallbacks},encode=${metrics.peakEncoderQueue},verify=${metrics.peakVerifierQueue},verifyCallbacks=${metrics.peakVerifierCallbacks}`);
    }
  }

  globalThis.__DIAXUS_M1__ = { run };

  if (new URLSearchParams(location.search).has("autorun")) {
    const waitForElement = setInterval(() => {
      const button = document.getElementById("run");
      if (!button) return;
      clearInterval(waitForElement);
      button.click();
    }, 10);
  }
})();
