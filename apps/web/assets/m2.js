import {
  BlobSource,
  BufferTarget,
  Input,
  MP4,
  Output,
  Quality,
  VideoSample,
  VideoSampleSink,
  VideoSampleSource,
  WEBM,
  WebMOutputFormat,
} from "mediabunny";

const MAX_INPUT_BYTES = 256 * 1024 * 1024;
let currentDownloadUrl = null;

function fail(message) {
  throw new Error(message);
}

function asSafeMicroseconds(value, label) {
  if (!Number.isSafeInteger(value)) {
    fail(`${label} is not a safe integer number of microseconds`);
  }
  return value;
}

async function openInput(file) {
  if (!(file instanceof File)) fail("Select an MP4 file first.");
  if (file.size === 0) fail("The selected file is empty.");
  if (file.size > MAX_INPUT_BYTES) {
    fail(`The selected file is ${(file.size / 1048576).toFixed(1)} MiB; M2 is limited to 256 MiB while output is buffered in memory.`);
  }

  const input = new Input({
    formats: [MP4],
    source: new BlobSource(file, { maxCacheSize: 8 * 1024 * 1024 }),
  });
  if (!(await input.canRead()) || (await input.getFormat()) !== MP4) {
    input.dispose();
    fail("M2 accepts ISO MP4 input only.");
  }

  const track = await input.getPrimaryVideoTrack();
  if (!track) {
    input.dispose();
    fail("The MP4 contains no video track.");
  }
  const codec = await track.getCodec();
  if (codec !== "avc") {
    input.dispose();
    fail(`M2 accepts H.264/AVC video only; selected track is ${codec ?? "unknown"}.`);
  }
  if (!(await track.canDecode())) {
    input.dispose();
    fail("This browser cannot decode the selected H.264 track with its exact codec configuration.");
  }

  const [width, height, rotation, flip, duration, stats, audioTracks, codecString] = await Promise.all([
    track.getCodedWidth(),
    track.getCodedHeight(),
    track.getRotation(),
    track.getFlip(),
    input.computeDuration([track]),
    track.computePacketStats(),
    input.getAudioTracks(),
    track.getCodecParameterString(),
  ]);
  if (rotation !== 0 || flip) {
    input.dispose();
    fail("M2 supports unrotated, unflipped MP4 video only; orientation transforms are scheduled for M3.");
  }
  return {
    input,
    track,
    width,
    height,
    duration,
    packetCount: stats.packetCount,
    averagePacketRate: stats.averagePacketRate,
    audioTracks: audioTracks.length,
    codecString: codecString ?? "avc",
  };
}

async function inspect(file) {
  const opened = await openInput(file);
  try {
    return {
      width: opened.width,
      height: opened.height,
      duration: opened.duration,
      packetCount: opened.packetCount,
      averagePacketRate: opened.averagePacketRate,
      audioTracks: opened.audioTracks,
      codec: opened.codecString,
      size: file.size,
      name: file.name,
    };
  } finally {
    opened.input.dispose();
  }
}

async function verifyWebM(buffer, expected) {
  const input = new Input({
    formats: [WEBM],
    source: new BlobSource(new Blob([buffer], { type: "video/webm" })),
  });
  try {
    if (!(await input.canRead())) fail("Finalized WebM could not be reopened by the container inspector.");
    const track = await input.getPrimaryVideoTrack();
    if (!track) fail("Finalized WebM has no video track.");
    const [codec, width, height, sampleDuration, metadataDuration, stats] = await Promise.all([
      track.getCodec(),
      track.getCodedWidth(),
      track.getCodedHeight(),
      input.computeDuration([track]),
      input.getDurationFromMetadata([track]),
      track.computePacketStats(),
    ]);
    if (codec !== "vp8") fail(`Finalized WebM codec is ${codec ?? "unknown"}, expected VP8.`);
    if (width !== expected.width || height !== expected.height) {
      fail(`Finalized WebM is ${width}×${height}, expected ${expected.width}×${expected.height}.`);
    }
    if (stats.packetCount !== expected.frameCount) {
      fail(`Finalized WebM contains ${stats.packetCount} packets for ${expected.frameCount} decoded input frames.`);
    }
    const tolerance = Math.max(0.050, expected.lastDurationSeconds + 0.010);
    const duration = metadataDuration ?? sampleDuration;
    if (Math.abs(duration - expected.durationSeconds) > tolerance || Math.abs(duration - sampleDuration) > tolerance) {
      fail(`Finalized WebM duration ${duration.toFixed(6)}s (sample coverage ${sampleDuration.toFixed(6)}s) differs from processed duration ${expected.durationSeconds.toFixed(6)}s.`);
    }

    // Exercise random-access decode at the midpoint. Independent ffprobe/player
    // validation is still required for the recorded M2 interoperability result.
    const midpoint = Math.max(0, duration / 2);
    const sample = await new VideoSampleSink(track).getSample(midpoint);
    if (!sample) fail("Finalized WebM could not seek/decode at its midpoint.");
    sample.close();
    return { duration, packetCount: stats.packetCount };
  } finally {
    input.dispose();
  }
}

async function run(file, outputWidth, outputHeight, processFrame, status, cancelled) {
  if (!globalThis.isSecureContext || !globalThis.VideoDecoder || !globalThis.VideoEncoder || !navigator.gpu) {
    fail("M2 requires a secure context, WebCodecs, and WebGPU.");
  }

  const opened = await openInput(file);
  let output = null;
  let outputStarted = false;
  let completed = false;
  let processed = 0;
  const startedAt = performance.now();
  try {
    if (opened.width < outputWidth || opened.height < outputHeight) {
      fail("The M2 half-size preset must not upscale the source.");
    }
    status(`Demuxed MP4/H.264: ${opened.width}×${opened.height}, ${opened.packetCount} packets. Audio tracks omitted: ${opened.audioTracks}.`);

    const target = new BufferTarget();
    let encodedPackets = 0;
    const source = new VideoSampleSource({
      codec: "vp8",
      quality: new Quality("high"),
      keyFrameInterval: 2,
      latencyMode: "quality",
      hardwareAcceleration: "no-preference",
      sizeChangeBehavior: "deny",
      onEncodedPacket: () => { encodedPackets += 1; },
    });
    output = new Output({ format: new WebMOutputFormat(), target });
    output.addVideoTrack(source, { name: "M2 resized video" });
    await output.start();
    outputStarted = true;

    const sink = new VideoSampleSink(opened.track);
    let firstTimestamp = null;
    let endTimestamp = 0;
    let lastDuration = 0;
    for await (const sample of sink.samples()) {
      if (cancelled()) throw new Error("CANCELLED: conversion stopped by user");
      const sourceTimestamp = asSafeMicroseconds(sample.microsecondTimestamp, "input timestamp");
      const reportedDuration = asSafeMicroseconds(sample.microsecondDuration, "input duration");
      const duration = reportedDuration > 0
        ? reportedDuration
        : Math.round(1_000_000 / opened.averagePacketRate);
      if (firstTimestamp === null) firstTimestamp = sourceTimestamp;
      const timestamp = sourceTimestamp - firstTimestamp;
      const decodedFrame = sample.toVideoFrame();
      sample.close();

      let processedFrame = null;
      try {
        processedFrame = await processFrame(decodedFrame, timestamp, duration);
        const outputSample = new VideoSample(processedFrame);
        try {
          await source.add(outputSample);
        } finally {
          outputSample.close();
        }
      } finally {
        decodedFrame.close();
        processedFrame?.close();
      }

      processed += 1;
      lastDuration = duration / 1_000_000;
      endTimestamp = (timestamp + duration) / 1_000_000;
      const percent = opened.packetCount > 0
        ? Math.min(99, Math.floor((processed / opened.packetCount) * 100))
        : Math.min(99, Math.floor((endTimestamp / opened.duration) * 100));
      status(`Converting ${percent}% — ${processed}/${opened.packetCount || "?"} frames; audio omitted.`);
    }

    if (cancelled()) throw new Error("CANCELLED: conversion stopped by user");
    status(`Finalizing WebM after ${processed} frames…`);
    await output.finalize();
    completed = true;
    if (!target.buffer) fail("WebM finalization produced no output buffer.");
    if (encodedPackets !== processed) {
      fail(`Encoder emitted ${encodedPackets} packets for ${processed} processed frames.`);
    }

    status("Inspecting finalized WebM and testing midpoint seek…");
    const verified = await verifyWebM(target.buffer, {
      width: outputWidth,
      height: outputHeight,
      frameCount: processed,
      durationSeconds: endTimestamp,
      lastDurationSeconds: lastDuration,
    });

    if (currentDownloadUrl) URL.revokeObjectURL(currentDownloadUrl);
    currentDownloadUrl = URL.createObjectURL(new Blob([target.buffer], { type: "video/webm" }));
    const base = file.name.replace(/\.[^.]+$/, "") || "converted";
    const elapsed = performance.now() - startedAt;
    return {
      summary: `PASS: ${processed} H.264 frames → ${outputWidth}×${outputHeight} VP8 WebM in ${elapsed.toFixed(1)} ms; ${verified.duration.toFixed(3)} s; audio omitted; conversion pixel readbacks=0.`,
      downloadUrl: currentDownloadUrl,
      fileName: `${base}-${outputWidth}x${outputHeight}.webm`,
      frameCount: processed,
      duration: verified.duration,
      outputBytes: target.buffer.byteLength,
    };
  } catch (error) {
    if (outputStarted && !completed) {
      try { await output.cancel(); } catch (_) { /* retain original failure */ }
    }
    throw error;
  } finally {
    opened.input.dispose();
  }
}

globalThis.__DIAXUS_M2__ = { inspect, run };
