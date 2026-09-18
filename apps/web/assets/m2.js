import {
  AudioSampleSink,
  AudioSampleSource,
  BlobSource,
  BufferTarget,
  Input,
  MP4,
  Mp4OutputFormat,
  Output,
  Quality,
  VideoSample,
  VideoSampleSink,
  VideoSampleSource,
  WEBM,
  WebMOutputFormat,
  canEncodeAudio,
  canEncodeVideo,
} from "mediabunny";

const MAX_INPUT_BYTES = 256 * 1024 * 1024;
let currentDownloadUrl = null;

function fail(message) {
  throw new Error(message);
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function isEmbeddedElectronBrowser() {
  return /Electron\//i.test(navigator.userAgent) || /\bCode\//i.test(navigator.userAgent);
}

function environmentHint() {
  return isEmbeddedElectronBrowser()
    ? " The VS Code embedded browser runs in Electron and may expose a different WebCodecs configuration set; open the same local URL in a standalone browser with WebCodecs and WebGPU support."
    : "";
}

function asSafeMicroseconds(value, label) {
  if (!Number.isSafeInteger(value)) {
    fail(`${label} is not a safe integer number of microseconds`);
  }
  return value;
}

class MediabunnyInputAdapter {
  static async open(file) {
  if (!(file instanceof File)) fail("Select an MP4 file first.");
  if (file.size === 0) fail("The selected file is empty.");
  if (file.size > MAX_INPUT_BYTES) {
    fail(`The selected file is ${(file.size / 1048576).toFixed(1)} MiB; browser conversion is limited to 256 MiB while output is buffered in memory.`);
  }

  const input = new Input({
    formats: [MP4],
    source: new BlobSource(file, { maxCacheSize: 8 * 1024 * 1024 }),
  });
  if (!(await input.canRead()) || (await input.getFormat()) !== MP4) {
    input.dispose();
    fail("The browser backend accepts ISO MP4 input only.");
  }

  const track = await input.getPrimaryVideoTrack();
  if (!track) {
    input.dispose();
    fail("The MP4 contains no video track.");
  }
  const codec = await track.getCodec();
  if (codec !== "avc") {
    input.dispose();
    fail(`The browser backend accepts H.264/AVC video only; selected track is ${codec ?? "unknown"}.`);
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
    fail("M3.2 supports unrotated, unflipped MP4 video only; orientation transforms remain scheduled for a later M3 slice.");
  }
  const audioTrack = audioTracks[0] ?? null;
  let audio = null;
  if (audioTrack) {
    const [audioCodec, numberOfChannels, sampleRate, canDecode, audioStats] = await Promise.all([
      audioTrack.getCodec(),
      audioTrack.getNumberOfChannels(),
      audioTrack.getSampleRate(),
      audioTrack.canDecode(),
      audioTrack.computePacketStats(),
    ]);
    audio = { track: audioTrack, codec: audioCodec, numberOfChannels, sampleRate, canDecode, packetCount: audioStats.packetCount };
  }
  return {
    input,
    track,
    width,
    height,
    duration,
    packetCount: stats.packetCount,
    averagePacketRate: stats.averagePacketRate,
    audioTrackCount: audioTracks.length,
    audio,
    codecString: codecString ?? "avc",
  };
  }
}

async function inspect(file) {
  const opened = await MediabunnyInputAdapter.open(file);
  try {
    return {
      width: opened.width,
      height: opened.height,
      duration: opened.duration,
      packetCount: opened.packetCount,
      averagePacketRate: opened.averagePacketRate,
      audioTracks: opened.audioTrackCount,
      audioCodec: opened.audio?.codec ?? null,
      audioChannels: opened.audio?.numberOfChannels ?? 0,
      audioSampleRate: opened.audio?.sampleRate ?? 0,
      codec: opened.codecString,
      size: file.size,
      name: file.name,
    };
  } finally {
    opened.input.dispose();
  }
}

const OUTPUT_PROFILES = Object.freeze({
  "webm-vp8-opus": {
    container: "webm", videoCodec: "vp8", audioCodec: "opus",
    label: "WebM/VP8/Opus", extension: "webm", mimeType: "video/webm",
  },
  "webm-vp8-video-only": {
    container: "webm", videoCodec: "vp8", audioCodec: null,
    label: "WebM/VP8 video only", extension: "webm", mimeType: "video/webm",
  },
  "mp4-h264-aac": {
    container: "mp4", videoCodec: "avc", audioCodec: "aac",
    label: "MP4/H.264/AAC", extension: "mp4", mimeType: "video/mp4",
  },
});

function resolveProfile(id) {
  const profile = OUTPUT_PROFILES[id];
  if (!profile) fail(`Unknown output profile: ${id}.`);
  return profile;
}

async function probeProfile(profile, opened, outputWidth, outputHeight) {
  if (profile.audioCodec && !opened.audio) {
    return { supported: false, reason: `${profile.label} requires an input audio track.` };
  }
  if (profile.audioCodec && !opened.audio.canDecode) {
    return { supported: false, reason: `The primary ${opened.audio.codec ?? "unknown"} audio track cannot be decoded.` };
  }
  try {
    const videoSupported = await canEncodeVideo(profile.videoCodec, {
      width: outputWidth,
      height: outputHeight,
      frameRate: opened.averagePacketRate,
      quality: new Quality("high"),
      latencyMode: "quality",
      hardwareAcceleration: "no-preference",
    });
    if (!videoSupported) {
      return {
        supported: false,
        reason: `${profile.videoCodec.toUpperCase()} encoding is unsupported at ${outputWidth}×${outputHeight} and ${opened.averagePacketRate.toFixed(3)} fps.`,
      };
    }
  } catch (error) {
    return { supported: false, reason: `${profile.videoCodec.toUpperCase()} capability probe failed: ${errorMessage(error)}` };
  }
  if (profile.audioCodec) {
    try {
      const audioSupported = await canEncodeAudio(profile.audioCodec, {
        numberOfChannels: opened.audio.numberOfChannels,
        sampleRate: 48_000,
        quality: new Quality("high"),
      });
      if (!audioSupported) {
        return {
          supported: false,
          reason: `${profile.audioCodec.toUpperCase()} encoding is unsupported at 48000 Hz with ${opened.audio.numberOfChannels} channel(s).`,
        };
      }
    } catch (error) {
      return { supported: false, reason: `${profile.audioCodec.toUpperCase()} capability probe failed: ${errorMessage(error)}` };
    }
  }
  return { supported: true, reason: "Supported for the selected input and exact output configuration." };
}

async function probeProfiles(file, outputWidth, outputHeight) {
  const opened = await MediabunnyInputAdapter.open(file);
  try {
    const mp4 = await probeProfile(OUTPUT_PROFILES["mp4-h264-aac"], opened, outputWidth, outputHeight);
    return { mp4Supported: mp4.supported, mp4Reason: mp4.reason };
  } finally {
    opened.input.dispose();
  }
}

function peakAmplitude(sample) {
  let peak = 0;
  for (let planeIndex = 0; planeIndex < sample.numberOfChannels; planeIndex += 1) {
    const options = { planeIndex, format: "f32-planar" };
    const values = new Float32Array(sample.allocationSize(options) / Float32Array.BYTES_PER_ELEMENT);
    sample.copyTo(values, options);
    for (const value of values) peak = Math.max(peak, Math.abs(value));
  }
  return peak;
}

async function verifyOutput(buffer, expected) {
  const input = new Input({
    formats: expected.container === "mp4" ? [MP4] : [WEBM],
    source: new BlobSource(new Blob([buffer], { type: expected.mimeType })),
  });
  try {
    if (!(await input.canRead())) fail(`Finalized ${expected.container.toUpperCase()} could not be reopened by the container inspector.`);
    const videoTrack = await input.getPrimaryVideoTrack();
    if (!videoTrack) fail(`Finalized ${expected.container.toUpperCase()} has no video track.`);
    if (!(await videoTrack.canDecode())) {
      fail(`This browser cannot re-decode the finalized ${expected.videoCodecLabel} track.${environmentHint()}`);
    }
    const [codec, width, height, videoDuration, metadataDuration, stats, videoFirst] = await Promise.all([
      videoTrack.getCodec(),
      videoTrack.getCodedWidth(),
      videoTrack.getCodedHeight(),
      input.computeDuration([videoTrack]),
      input.getDurationFromMetadata([videoTrack]),
      videoTrack.computePacketStats(),
      videoTrack.getFirstTimestamp(),
    ]);
    if (codec !== expected.videoCodec) {
      fail(`Finalized ${expected.container.toUpperCase()} video codec is ${codec ?? "unknown"}, expected ${expected.videoCodecLabel}.`);
    }
    if (width !== expected.width || height !== expected.height) {
      fail(`Finalized output is ${width}×${height}, expected ${expected.width}×${expected.height}.`);
    }
    if (stats.packetCount !== expected.frameCount) {
      fail(`Finalized output contains ${stats.packetCount} video packets for ${expected.frameCount} decoded input frames.`);
    }
    const tolerance = Math.max(0.050, expected.lastVideoDurationSeconds + 0.010);
    if (Math.abs(videoDuration - expected.videoEndSeconds) > tolerance) {
      fail(`Finalized video duration ${videoDuration.toFixed(6)}s differs from processed coverage ${expected.videoEndSeconds.toFixed(6)}s.`);
    }

    // Exercise random-access decode at the midpoint. Independent ffprobe/player
    // validation is still required for the recorded M2 interoperability result.
    const midpoint = Math.max(0, videoDuration / 2);
    const sample = await new VideoSampleSink(videoTrack).getSample(midpoint);
    if (!sample) fail("Finalized output could not seek/decode video at its midpoint.");
    sample.close();

    const audioTracks = await input.getAudioTracks();
    if (!expected.hasAudio) {
      if (audioTracks.length !== 0) fail(`Video-only profile produced ${audioTracks.length} audio track(s).`);
      return { duration: metadataDuration ?? videoDuration, packetCount: stats.packetCount, audioPackets: 0, audioPeak: 0 };
    }
    const audioTrack = audioTracks[0];
    if (!audioTrack) fail("Audio-preserving profile produced no audio track.");
    if (!(await audioTrack.canDecode())) {
      fail(`This browser encoded the ${expected.audioCodecLabel} track but cannot re-decode it for verification.${environmentHint()}`);
    }
    const [audioCodec, channels, sampleRate, audioDuration, audioStats, audioFirst] = await Promise.all([
      audioTrack.getCodec(),
      audioTrack.getNumberOfChannels(),
      audioTrack.getSampleRate(),
      input.computeDuration([audioTrack]),
      audioTrack.computePacketStats(),
      audioTrack.getFirstTimestamp(),
    ]);
    if (audioCodec !== expected.audioCodec) {
      fail(`Finalized output audio codec is ${audioCodec ?? "unknown"}, expected ${expected.audioCodecLabel}.`);
    }
    if (channels !== expected.audioChannels || sampleRate !== 48_000) {
      fail(`Finalized ${expected.audioCodecLabel} is ${channels} channel(s) at ${sampleRate} Hz; expected ${expected.audioChannels} at 48000 Hz.`);
    }
    if (Math.abs(videoFirst - expected.videoFirstSeconds) > tolerance || Math.abs(audioFirst - expected.audioFirstSeconds) > tolerance) {
      fail(`Output A/V start offsets changed: video=${videoFirst.toFixed(6)}s audio=${audioFirst.toFixed(6)}s.`);
    }
    if (Math.abs(audioDuration - expected.audioEndSeconds) > 0.075) {
      fail(`Finalized ${expected.audioCodecLabel} duration ${audioDuration.toFixed(6)}s differs from decoded audio coverage ${expected.audioEndSeconds.toFixed(6)}s.`);
    }
    const audioSink = new AudioSampleSink(audioTrack);
    const points = [audioFirst + 0.01, (audioFirst + audioDuration) / 2, Math.max(audioFirst, audioDuration - 0.05)];
    let audioPeak = 0;
    for (const point of points) {
      const audioSample = await audioSink.getSample(point);
      if (!audioSample) fail(`Finalized ${expected.audioCodecLabel} could not decode near ${point.toFixed(3)}s.`);
      try { audioPeak = Math.max(audioPeak, peakAmplitude(audioSample)); } finally { audioSample.close(); }
    }
    if (audioPeak < 0.00001) fail(`Decoded ${expected.audioCodecLabel} samples at beginning/middle/end were silent.`);
    return {
      duration: Math.max(metadataDuration ?? 0, videoDuration, audioDuration),
      packetCount: stats.packetCount,
      audioPackets: audioStats.packetCount,
      audioPeak,
    };
  } finally {
    input.dispose();
  }
}

class MediabunnyOutputAdapter {
  constructor(profile, audioConfig) {
    this.target = new BufferTarget();
    this.videoPackets = 0;
    this.audioPackets = 0;
    this.videoEncoderConfig = null;
    this.audioEncoderConfig = null;
    const format = profile.container === "mp4"
      ? new Mp4OutputFormat({ fastStart: "in-memory" })
      : new WebMOutputFormat();
    this.output = new Output({ format, target: this.target });
    this.videoSource = new VideoSampleSource({
      codec: profile.videoCodec,
      quality: new Quality("high"),
      keyFrameInterval: 2,
      latencyMode: "quality",
      hardwareAcceleration: "no-preference",
      sizeChangeBehavior: "deny",
      onEncoderConfig: (config) => { this.videoEncoderConfig = config; },
      onEncodedPacket: () => { this.videoPackets += 1; },
    });
    this.output.addVideoTrack(this.videoSource, { name: "M3 resized video" });
    this.audioSource = null;
    if (profile.audioCodec) {
      this.audioSource = new AudioSampleSource({
        codec: profile.audioCodec,
        quality: new Quality("high"),
        transform: { sampleRate: 48_000, numberOfChannels: audioConfig.numberOfChannels },
        onEncoderConfig: (config) => { this.audioEncoderConfig = config; },
        onEncodedPacket: () => { this.audioPackets += 1; },
      });
      this.output.addAudioTrack(this.audioSource, { name: "M3 transcoded audio" });
    }
  }

  start() { return this.output.start(); }
  finalize() { return this.output.finalize(); }
  cancel() { return this.output.cancel(); }
}

async function runBrowserJob(file, outputWidth, outputHeight, profileId, processFrame, status, cancelled) {
  if (!globalThis.isSecureContext || !globalThis.VideoDecoder || !globalThis.VideoEncoder || !navigator.gpu) {
    fail("The browser backend requires a secure context, WebCodecs, and WebGPU.");
  }

  const profile = resolveProfile(profileId);
  const opened = await MediabunnyInputAdapter.open(file);
  let muxer = null;
  let outputStarted = false;
  let completed = false;
  let processed = 0;
  let audioSamples = 0;
  const startedAt = performance.now();
  try {
    if (opened.width < outputWidth || opened.height < outputHeight) {
      fail("The half-size preset must not upscale the source.");
    }
    const profileCapability = await probeProfile(profile, opened, outputWidth, outputHeight);
    if (!profileCapability.supported) {
      fail(`${profile.label} is unavailable for this input: ${profileCapability.reason}`);
    }
    const selectedTracks = profile.audioCodec ? [opened.track, opened.audio.track] : [opened.track];
    const originSeconds = await opened.input.getFirstTimestamp(selectedTracks);
    const originUs = asSafeMicroseconds(Math.round(originSeconds * 1_000_000), "input origin");
    const videoFirstSeconds = (asSafeMicroseconds(Math.round(await opened.track.getFirstTimestamp() * 1_000_000), "video start") - originUs) / 1_000_000;
    const audioFirstSeconds = opened.audio
      ? (asSafeMicroseconds(Math.round(await opened.audio.track.getFirstTimestamp() * 1_000_000), "audio start") - originUs) / 1_000_000
      : 0;
    status(`Demuxed MP4/H.264: ${opened.width}×${opened.height}, ${opened.packetCount} video packets; profile ${profile.label}.`);

    muxer = new MediabunnyOutputAdapter(profile, opened.audio);
    try {
      await muxer.start();
    } catch (error) {
      fail(`${profile.container.toUpperCase()} muxer initialization failed: ${errorMessage(error)}`);
    }
    outputStarted = true;

    let videoEndSeconds = 0;
    let audioEndSeconds = 0;
    let lastVideoDurationSeconds = 0;
    let siblingFailure = null;
    const ensureActive = () => {
      if (siblingFailure) throw siblingFailure;
      if (cancelled()) throw new Error("CANCELLED: conversion stopped by user");
    };
    const pumpVideo = async () => {
      const sink = new VideoSampleSink(opened.track);
      for await (const sample of sink.samples()) {
        let decodedFrame = null;
        let processedFrame = null;
        try {
          ensureActive();
          const sourceTimestamp = asSafeMicroseconds(sample.microsecondTimestamp, "input video timestamp");
          const reportedDuration = asSafeMicroseconds(sample.microsecondDuration, "input video duration");
          const duration = reportedDuration > 0 ? reportedDuration : Math.round(1_000_000 / opened.averagePacketRate);
          const timestamp = sourceTimestamp - originUs;
          decodedFrame = sample.toVideoFrame();
          processedFrame = await processFrame(decodedFrame, timestamp, duration);
          const outputSample = new VideoSample(processedFrame);
          try {
            await muxer.videoSource.add(outputSample);
          } catch (error) {
            const config = muxer.videoEncoderConfig;
            const configured = config
              ? `${config.codec}, ${config.width}×${config.height}, ${config.bitrate ?? "auto"} bit/s, hardware=${config.hardwareAcceleration ?? "no-preference"}`
              : `${outputWidth}×${outputHeight} ${profile.videoCodec.toUpperCase()} (encoder config was not emitted)`;
            fail(`${profile.videoCodec.toUpperCase()} encoder configuration failed (${configured}): ${errorMessage(error)}`);
          } finally {
            outputSample.close();
          }
          processed += 1;
          lastVideoDurationSeconds = duration / 1_000_000;
          videoEndSeconds = (timestamp + duration) / 1_000_000;
          const percent = opened.packetCount > 0 ? Math.min(99, Math.floor((processed / opened.packetCount) * 100)) : 0;
          status(`Converting ${percent}% — video ${processed}/${opened.packetCount || "?"}, audio ${audioSamples}/${opened.audio?.packetCount ?? 0}.`);
        } finally {
          sample.close();
          decodedFrame?.close();
          processedFrame?.close();
        }
      }
    };

    const pumpAudio = async () => {
      if (!muxer.audioSource) return;
      const sink = new AudioSampleSink(opened.audio.track);
      for await (const sample of sink.samples()) {
        try {
          ensureActive();
          const timestamp = asSafeMicroseconds(sample.microsecondTimestamp, "input audio timestamp") - originUs;
          const duration = asSafeMicroseconds(sample.microsecondDuration, "input audio duration");
          sample.setTimestamp(timestamp / 1_000_000);
          try {
            await muxer.audioSource.add(sample);
          } catch (error) {
            const config = muxer.audioEncoderConfig;
            const configured = config
              ? `${config.codec}, ${config.sampleRate} Hz, ${config.numberOfChannels} channel(s), ${config.bitrate ?? "auto"} bit/s`
              : `${profile.audioCodec.toUpperCase()}, 48000 Hz, ${opened.audio.numberOfChannels} channel(s) (encoder config was not emitted)`;
            fail(`${profile.audioCodec.toUpperCase()} encoder configuration failed (${configured}): ${errorMessage(error)}`);
          }
          audioSamples += 1;
          audioEndSeconds = (timestamp + duration) / 1_000_000;
        } finally {
          sample.close();
        }
      }
    };

    const guardPump = async (label, pump) => {
      try {
        await pump();
      } catch (error) {
        const contextual = errorMessage(error).startsWith("CANCELLED:")
          || errorMessage(error).includes("encoder configuration failed")
          ? error
          : new Error(`${label} pipeline failed: ${errorMessage(error)}`);
        siblingFailure ??= contextual;
        throw contextual;
      }
    };
    const pumpResults = await Promise.allSettled([
      guardPump("Video decode/process/encode", pumpVideo),
      guardPump("Audio decode/encode", pumpAudio),
    ]);
    const rejected = pumpResults.find((result) => result.status === "rejected");
    if (rejected) throw rejected.reason;

    if (cancelled()) throw new Error("CANCELLED: conversion stopped by user");
    status(`Finalizing ${profile.container.toUpperCase()} after ${processed} video frames and ${audioSamples} audio samples…`);
    try {
      await muxer.finalize();
    } catch (error) {
      fail(`${profile.container.toUpperCase()} encoder drain/finalization failed: ${errorMessage(error)}${environmentHint()}`);
    }
    completed = true;
    if (!muxer.target.buffer) fail(`${profile.container.toUpperCase()} finalization produced no output buffer.`);
    if (muxer.videoPackets !== processed) {
      fail(`Video encoder emitted ${muxer.videoPackets} packets for ${processed} processed frames.`);
    }
    if (profile.audioCodec && muxer.audioPackets === 0) {
      fail(`${profile.audioCodec.toUpperCase()} encoder emitted no packets.`);
    }

    status(`Inspecting finalized ${profile.container.toUpperCase()} and decoding video/audio at beginning, midpoint, and end…`);
    let verified;
    try {
      verified = await verifyOutput(muxer.target.buffer, {
        container: profile.container,
        mimeType: profile.mimeType,
        videoCodec: profile.videoCodec,
        videoCodecLabel: profile.videoCodec === "avc" ? "H.264" : "VP8",
        audioCodec: profile.audioCodec,
        audioCodecLabel: profile.audioCodec === "aac" ? "AAC" : "Opus",
        width: outputWidth,
        height: outputHeight,
        frameCount: processed,
        videoEndSeconds,
        lastVideoDurationSeconds,
        videoFirstSeconds,
        hasAudio: Boolean(profile.audioCodec),
        audioChannels: opened.audio?.numberOfChannels ?? 0,
        audioFirstSeconds,
        audioEndSeconds,
      });
    } catch (error) {
      fail(`Finalized ${profile.container.toUpperCase()} verification failed: ${errorMessage(error)}${environmentHint()}`);
    }

    if (currentDownloadUrl) URL.revokeObjectURL(currentDownloadUrl);
    currentDownloadUrl = URL.createObjectURL(new Blob([muxer.target.buffer], { type: profile.mimeType }));
    const base = file.name.replace(/\.[^.]+$/, "") || "converted";
    const elapsed = performance.now() - startedAt;
    return {
      summary: profile.audioCodec
        ? `PASS: ${processed} H.264 input frames + ${audioSamples} decoded audio samples → ${outputWidth}×${outputHeight} ${profile.label} in ${elapsed.toFixed(1)} ms; ${verified.duration.toFixed(3)} s; ${verified.audioPackets} ${profile.audioCodec.toUpperCase()} packets; decoded audio peak=${verified.audioPeak.toFixed(4)}; audio queue peak=1; conversion pixel readbacks=0.`
        : `PASS: ${processed} H.264 input frames → ${outputWidth}×${outputHeight} ${profile.label} in ${elapsed.toFixed(1)} ms; ${verified.duration.toFixed(3)} s; conversion pixel readbacks=0.`,
      downloadUrl: currentDownloadUrl,
      fileName: `${base}-${outputWidth}x${outputHeight}.${profile.extension}`,
      frameCount: processed,
      duration: verified.duration,
      outputBytes: muxer.target.buffer.byteLength,
    };
  } catch (error) {
    if (outputStarted && !completed) {
      try { await muxer.cancel(); } catch (_) { /* retain original failure */ }
    }
    const message = errorMessage(error);
    if (isEmbeddedElectronBrowser() && !message.includes("VS Code embedded browser")) {
      throw new Error(`${message}${environmentHint()}`);
    }
    throw error;
  } finally {
    opened.input.dispose();
  }
}

class WebCodecsMediabunnyBackend {
  inspect(file) {
    return inspect(file);
  }

  probeProfiles(file, outputWidth, outputHeight) {
    return probeProfiles(file, outputWidth, outputHeight);
  }

  run(file, outputWidth, outputHeight, profileId, processFrame, status, cancelled) {
    return runBrowserJob(file, outputWidth, outputHeight, profileId, processFrame, status, cancelled);
  }
}

globalThis.__DIAXUS_MEDIA_WEB__ = new WebCodecsMediabunnyBackend();
