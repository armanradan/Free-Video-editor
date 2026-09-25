import {
  AudioSampleSink,
  AudioSampleSource,
  BlobSource,
  BufferTarget,
  EncodedPacketSink,
  Input,
  MP4,
  Mp4OutputFormat,
  Output,
  Quality,
  StreamTarget,
  VideoSample,
  VideoSampleSink,
  VideoSampleSource,
  WEBM,
  WebMOutputFormat,
  canEncodeAudio,
  canEncodeVideo,
} from "mediabunny";

const MEMORY_FALLBACK_MAX_INPUT_BYTES = 256 * 1024 * 1024;
const OUTPUT_CHUNK_BYTES = 4 * 1024 * 1024;
const INPUT_CACHE_BYTES = 8 * 1024 * 1024;
// The WASM build still has a finite address space; this is a disk-spool bound,
// not an application-memory cap. Quota is checked separately at job startup.
const FFMPEG_MAX_RAW_SPOOL_BYTES = 2 * 1024 * 1024 * 1024;
const ffmpegSelected = () => globalThis.__DIAXUS_BACKEND__ === "ffmpeg-wasm";
const OUTPUT_INSTANCE_ID = globalThis.crypto?.randomUUID?.()
  ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`;
const OPFS_OUTPUT_NAME = `diaxus-${OUTPUT_INSTANCE_ID}.partial`;
const OPFS_RAW_NAME = `diaxus-${OUTPUT_INSTANCE_ID}.rgba.partial`;
let retainedOutputStorage = null;
const consumedFailureInjections = new Set();

async function cleanupRetainedOutput() {
  const storage = retainedOutputStorage;
  retainedOutputStorage = null;
  if (!storage?.root || !storage.name) return;
  try { await storage.root.removeEntry(storage.name); }
  catch (_) { /* already removed or evicted */ }
}

async function removeOpfsEntry(root, name) {
  let lastError;
  for (let attempt = 0; attempt < 20; attempt += 1) {
    try { await root.removeEntry(name); return; }
    catch (error) {
      if (error?.name === "NotFoundError") return;
      lastError = error;
      await new Promise(resolve => setTimeout(resolve, 25));
    }
  }
  fail(`Could not release temporary OPFS entry ${name}: ${errorMessage(lastError)}`);
}

// Shared browser storage policy for both codec backends. Only WebCodecs has a
// bounded memory fallback; FFmpeg requires disk-backed ingress and egress.
const OutputStorageKind = Object.freeze({ OPFS: "opfs", MEMORY: "memory" });

async function createOutputStorage(outputMode, requireOpfs = false) {
  try {
    if (outputMode === "memory") fail("memory compatibility mode was explicitly requested");
    if (!navigator.storage?.getDirectory) fail("origin-private file storage is unavailable");
    const root = await navigator.storage.getDirectory();
    await cleanupRetainedOutput();
    const handle = await root.getFileHandle(OPFS_OUTPUT_NAME, { create: true });
    return { kind: OutputStorageKind.OPFS, root, handle, name: OPFS_OUTPUT_NAME, fallbackReason: null };
  } catch (error) {
    if (requireOpfs) fail(`FFmpeg WASM needs origin-private file streaming: ${errorMessage(error)}`);
    return { kind: OutputStorageKind.MEMORY, root: null, handle: null, name: null, fallbackReason: errorMessage(error) };
  }
}

function ffmpegFrameRate(rate) {
  if (Number.isInteger(rate) && rate >= 1 && rate <= 120) return { num: rate, den: 1 };
  for (const num of [24000, 30000, 60000]) {
    if (Math.abs(rate - num / 1001) < 0.005) return { num, den: 1001 };
  }
  return null;
}

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

function normalizedColorSpace(value) {
  return {
    primaries: value?.primaries ?? null,
    transfer: value?.transfer ?? null,
    matrix: value?.matrix ?? null,
    fullRange: value?.fullRange ?? null,
  };
}

function validateSdrColorSpace(colorSpace, hasHdr) {
  const color = normalizedColorSpace(colorSpace);
  if (hasHdr) {
    fail(`HDR input is not supported by the M3.5 SDR pipeline (${color.primaries ?? "unknown"}/${color.transfer ?? "unknown"}/${color.matrix ?? "unknown"}).`);
  }
  const allowedPrimaries = new Set([null, "bt709"]);
  const allowedTransfer = new Set([null, "bt709", "iec61966-2-1"]);
  const allowedMatrix = new Set([null, "bt709", "rgb"]);
  if (!allowedPrimaries.has(color.primaries)
      || !allowedTransfer.has(color.transfer)
      || !allowedMatrix.has(color.matrix)) {
    fail(`Unsupported SDR color space ${color.primaries ?? "unknown"}/${color.transfer ?? "unknown"}/${color.matrix ?? "unknown"}; M3.5 accepts BT.709/sRGB SDR only.`);
  }
  return color;
}

class MediabunnyInputAdapter {
  static async open(file) {
  if (!(file instanceof File)) fail("Select an MP4 file first.");
  if (file.size === 0) fail("The selected file is empty.");
  const input = new Input({
    formats: [MP4],
    source: new BlobSource(file, { maxCacheSize: INPUT_CACHE_BYTES }),
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
  if (codec !== "avc" && codec !== "hevc") {
    input.dispose();
    fail(`The browser backend accepts H.264/AVC or H.265/HEVC video; selected track is ${codec ?? "unknown"}.`);
  }
  if (!(await track.canDecode())) {
    input.dispose();
    fail(`This browser cannot decode the selected ${codec === "hevc" ? "H.265/HEVC" : "H.264/AVC"} track with its exact codec configuration.`);
  }

  const [trackCodedWidth, trackCodedHeight, squarePixelWidth, squarePixelHeight, displayWidth, displayHeight,
    pixelAspectRatio, rotation, flip, colorSpace, hasHdr, canBeTransparent, duration, stats, audioTracks, codecString] = await Promise.all([
    track.getCodedWidth(),
    track.getCodedHeight(),
    track.getSquarePixelWidth(),
    track.getSquarePixelHeight(),
    track.getDisplayWidth(),
    track.getDisplayHeight(),
    track.getPixelAspectRatio(),
    track.getRotation(),
    track.getFlip(),
    track.getColorSpace(),
    track.hasHighDynamicRange(),
    track.canBeTransparent(),
    input.computeDuration([track]),
    track.computePacketStats(),
    input.getAudioTracks(),
    track.getCodecParameterString(),
  ]);
  if (![0, 90, 180, 270].includes(rotation)) {
    input.dispose();
    fail(`Unsupported video rotation ${rotation}; expected a multiple of 90 degrees.`);
  }
  if (![trackCodedWidth, trackCodedHeight, squarePixelWidth, squarePixelHeight, displayWidth, displayHeight]
      .every(value => Number.isSafeInteger(value) && value > 0)) {
    input.dispose();
    fail("Video geometry contains a non-positive or non-integer dimension.");
  }
  if (displayWidth !== (rotation % 180 === 0 ? squarePixelWidth : squarePixelHeight)
      || displayHeight !== (rotation % 180 === 0 ? squarePixelHeight : squarePixelWidth)) {
    input.dispose();
    fail(`Inconsistent display geometry: square-pixel ${squarePixelWidth}×${squarePixelHeight}, rotation ${rotation}°, display ${displayWidth}×${displayHeight}.`);
  }
  if (!Number.isSafeInteger(pixelAspectRatio.num) || pixelAspectRatio.num <= 0
      || !Number.isSafeInteger(pixelAspectRatio.den) || pixelAspectRatio.den <= 0) {
    input.dispose();
    fail("Video pixel aspect ratio is invalid.");
  }
  if (canBeTransparent) {
    input.dispose();
    fail("Transparent video is not supported by the opaque M3.5 SDR output policy.");
  }
  let color;
  try { color = validateSdrColorSpace(colorSpace, hasHdr); }
  catch (error) { input.dispose(); throw error; }
  let firstSample;
  try {
    firstSample = await new VideoSampleSink(track).getSample(await track.getFirstTimestamp());
    if (!firstSample) fail("The primary video track produced no decodable geometry sample.");
    if (firstSample.rotation !== rotation || firstSample.flip !== flip) {
      fail(`Track/sample orientation mismatch: track=${rotation}° flip=${flip}, sample=${firstSample.rotation}° flip=${firstSample.flip}.`);
    }
    if (firstSample.squarePixelWidth !== squarePixelWidth || firstSample.squarePixelHeight !== squarePixelHeight) {
      fail(`Track/sample square-pixel geometry mismatch: track=${squarePixelWidth}×${squarePixelHeight}, sample=${firstSample.squarePixelWidth}×${firstSample.squarePixelHeight}.`);
    }
    color = validateSdrColorSpace(firstSample.colorSpace, false);
  } catch (error) {
    firstSample?.close();
    input.dispose();
    throw error;
  }
  const visibleRect = { ...firstSample.visibleRect };
  // The container's track dimensions can describe the clean aperture while the
  // decoder exposes the larger coded allocation. Use the decoded sample as the
  // authoritative coded grid so non-zero crop offsets remain representable.
  const codedWidth = firstSample.codedWidth;
  const codedHeight = firstSample.codedHeight;
  firstSample.close();
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
    fileSize: file.size,
    track,
    codedWidth,
    codedHeight,
    width: squarePixelWidth,
    height: squarePixelHeight,
    displayWidth,
    displayHeight,
    pixelAspectRatio,
    rotation,
    flip,
    color,
    visibleRect,
    duration,
    packetCount: stats.packetCount,
    averagePacketRate: stats.averagePacketRate,
    audioTrackCount: audioTracks.length,
    audio,
    codec,
    codecString: codecString ?? codec,
  };
  }
}

async function inspect(file) {
  const opened = await MediabunnyInputAdapter.open(file);
  try {
    return {
      width: opened.width,
      height: opened.height,
      codedWidth: opened.codedWidth,
      codedHeight: opened.codedHeight,
      visibleX: opened.visibleRect.left,
      visibleY: opened.visibleRect.top,
      visibleWidth: opened.visibleRect.width,
      visibleHeight: opened.visibleRect.height,
      displayWidth: opened.displayWidth,
      displayHeight: opened.displayHeight,
      rotation: opened.rotation,
      flip: opened.flip,
      pixelAspectNumerator: opened.pixelAspectRatio.num,
      pixelAspectDenominator: opened.pixelAspectRatio.den,
      colorPrimaries: opened.color.primaries ?? "unknown",
      colorTransfer: opened.color.transfer ?? "unknown",
      colorMatrix: opened.color.matrix ?? "unknown",
      colorFullRange: opened.color.fullRange,
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
  "mp4-h265-aac": {
    container: "mp4", videoCodec: "hevc", audioCodec: "aac",
    label: "MP4/H.265/AAC", extension: "mp4", mimeType: "video/mp4",
  },
});

function resolveProfile(id) {
  const profile = OUTPUT_PROFILES[id];
  if (!profile) fail(`Unknown output profile: ${id}.`);
  return profile;
}

async function probeProfile(profile, opened, outputWidth, outputHeight, hardwareAcceleration = "no-preference") {
  const ffmpeg = ffmpegSelected();
  if (ffmpeg && profile.container !== "mp4") {
    return { supported: false, reason: "The FFmpeg WASM spike supports MP4/H.264/AAC only; choose the WebCodecs backend for this profile." };
  }
  if (ffmpeg && profile.videoCodec !== "avc") {
    return { supported: false, reason: "The FFmpeg WASM spike does not enable H.265 output." };
  }
  if (profile.audioCodec && !opened.audio) {
    return { supported: false, reason: `${profile.label} requires an input audio track.` };
  }
  if (profile.audioCodec && !opened.audio.canDecode) {
    return { supported: false, reason: `The primary ${opened.audio.codec ?? "unknown"} audio track cannot be decoded.` };
  }
  try {
    const decoderConfig = await opened.track.getDecoderConfig();
    if (!decoderConfig) {
      return { supported: false, reason: "The input video track returned no WebCodecs decoder configuration." };
    }
    const decoderSupport = await VideoDecoder.isConfigSupported({
      ...decoderConfig,
      hardwareAcceleration,
    });
    if (!decoderSupport.supported) {
      return {
        supported: false,
        reason: `${opened.codec === "hevc" ? "H.265/HEVC" : "H.264/AVC"} decoding is unsupported with hardwareAcceleration=${hardwareAcceleration}.`,
      };
    }
  } catch (error) {
    return { supported: false, reason: `Input video decoder capability probe failed with hardwareAcceleration=${hardwareAcceleration}: ${errorMessage(error)}` };
  }
  if (ffmpeg) {
    if (!globalThis.__DIAXUS_FFMPEG_ASSETS__) return { supported: false, reason: "FFmpeg WASM assets were not configured." };
    if (!navigator.storage?.getDirectory) return { supported: false, reason: "FFmpeg WASM requires origin-private file streaming, which this browser does not expose." };
    if (opened.audioTrackCount !== 1) return { supported: false, reason: "FFmpeg WASM spike requires exactly one input audio track." };
    if (!ffmpegFrameRate(opened.averagePacketRate)) {
      return { supported: false, reason: "FFmpeg WASM raw-video bridge requires a constant integer frame rate from 1–120 fps or 24000/1001, 30000/1001, 60000/1001." };
    }
    const rawBytes = opened.packetCount * outputWidth * outputHeight * 4;
    if (!Number.isSafeInteger(rawBytes) || rawBytes > FFMPEG_MAX_RAW_SPOOL_BYTES) {
      return { supported: false, reason: "FFmpeg WASM raw-frame spool would exceed its 2 GiB disk limit; choose a smaller output resolution or WebCodecs." };
    }
    return { supported: true, reason: "Experimental FFmpeg WASM MP4/H.264/AAC with disk-backed raw-frame and output streaming; storage quota and exact frame timing are checked during conversion." };
  }
  try {
    const videoSupported = await canEncodeVideo(profile.videoCodec, {
      width: outputWidth,
      height: outputHeight,
      frameRate: opened.averagePacketRate,
      quality: new Quality("high"),
      latencyMode: "quality",
      hardwareAcceleration,
    });
    if (!videoSupported) {
      return {
        supported: false,
        reason: `${profile.videoCodec.toUpperCase()} encoding is unsupported at ${outputWidth}×${outputHeight}, ${opened.averagePacketRate.toFixed(3)} fps, hardwareAcceleration=${hardwareAcceleration}.`,
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
    const hevc = await probeProfile(OUTPUT_PROFILES["mp4-h265-aac"], opened, outputWidth, outputHeight);
    return {
      mp4Supported: mp4.supported,
      mp4Reason: mp4.reason,
      hevcSupported: hevc.supported,
      hevcReason: hevc.reason,
    };
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

async function verifyOutput(blob, expected) {
  const input = new Input({
    formats: expected.container === "mp4" ? [MP4] : [WEBM],
    source: new BlobSource(blob),
  });
  try {
    if (!(await input.canRead())) fail(`Finalized ${expected.container.toUpperCase()} could not be reopened by the container inspector.`);
    const videoTrack = await input.getPrimaryVideoTrack();
    if (!videoTrack) fail(`Finalized ${expected.container.toUpperCase()} has no video track.`);
    if (!(await videoTrack.canDecode())) {
      fail(`This browser cannot re-decode the finalized ${expected.videoCodecLabel} track.${environmentHint()}`);
    }
    const [codec, width, height, displayWidth, displayHeight, rotation, flip, pixelAspectRatio,
      colorSpace, hasHdr, videoDuration, metadataDuration, stats, videoFirst] = await Promise.all([
      videoTrack.getCodec(),
      videoTrack.getCodedWidth(),
      videoTrack.getCodedHeight(),
      videoTrack.getDisplayWidth(),
      videoTrack.getDisplayHeight(),
      videoTrack.getRotation(),
      videoTrack.getFlip(),
      videoTrack.getPixelAspectRatio(),
      videoTrack.getColorSpace(),
      videoTrack.hasHighDynamicRange(),
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
    if (displayWidth !== expected.width || displayHeight !== expected.height
        || rotation !== 0 || flip || pixelAspectRatio.num !== 1 || pixelAspectRatio.den !== 1) {
      fail(`Finalized output geometry is coded=${width}×${height}, display=${displayWidth}×${displayHeight}, rotation=${rotation}°, flip=${flip}, PAR=${pixelAspectRatio.num}:${pixelAspectRatio.den}; expected baked orientation with square pixels at ${expected.width}×${expected.height}.`);
    }
    validateSdrColorSpace(colorSpace, hasHdr);
    if (stats.packetCount !== expected.frameCount) {
      fail(`Finalized output contains ${stats.packetCount} video packets for ${expected.frameCount} decoded input frames.`);
    }
    const outputTimeline = [];
    for await (const packet of new EncodedPacketSink(videoTrack).packets(undefined, undefined, { metadataOnly: true })) {
      outputTimeline.push({ timestamp: packet.microsecondTimestamp, duration: packet.microsecondDuration });
    }
    outputTimeline.sort((a, b) => a.timestamp - b.timestamp);
    if (outputTimeline.length !== expected.videoTimeline.length) {
      fail(`Finalized output timeline has ${outputTimeline.length} packets, expected ${expected.videoTimeline.length}.`);
    }
    // WebM uses millisecond timecode ticks here; a packet duration is the
    // difference between two independently rounded endpoints.
    const timelineToleranceUs = expected.container === "webm" ? 1_000 : 5;
    let maxTimelineErrorUs = 0;
    for (let index = 0; index < outputTimeline.length; index++) {
      const actual = outputTimeline[index];
      const wanted = expected.videoTimeline[index];
      const timestampError = Math.abs(actual.timestamp - wanted.timestamp);
      // WebM may omit a packet duration when the next timestamp defines the
      // display interval. Only the final packet lacks that adjacent endpoint;
      // the separately checked track coverage provides its duration.
      const effectiveDuration = actual.duration === 0 && index + 1 < outputTimeline.length
        ? outputTimeline[index + 1].timestamp - actual.timestamp
        : actual.duration;
      const durationError = index === outputTimeline.length - 1 && effectiveDuration === 0
        ? 0
        : Math.abs(effectiveDuration - wanted.duration);
      maxTimelineErrorUs = Math.max(maxTimelineErrorUs, timestampError, durationError);
      if (timestampError > timelineToleranceUs || durationError > timelineToleranceUs) {
        fail(`Finalized output timeline differs beyond the ${timelineToleranceUs} µs ${expected.container.toUpperCase()} tick tolerance at frame ${index}: ${actual.timestamp}+${effectiveDuration} µs, expected ${wanted.timestamp}+${wanted.duration} µs.`);
      }
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
    try {
      if (sample.rotation !== 0 || sample.flip || sample.displayWidth !== expected.width || sample.displayHeight !== expected.height) {
        fail(`Decoded output sample did not retain baked square-pixel geometry at ${expected.width}×${expected.height}.`);
      }
      validateSdrColorSpace(sample.colorSpace, false);
    } finally { sample.close(); }

    const audioTracks = await input.getAudioTracks();
    if (!expected.hasAudio) {
      if (audioTracks.length !== 0) fail(`Video-only profile produced ${audioTracks.length} audio track(s).`);
      return { duration: metadataDuration ?? videoDuration, packetCount: stats.packetCount, timelinePackets: outputTimeline.length, maxTimelineErrorUs, audioPackets: 0, audioPeak: 0 };
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
      timelinePackets: outputTimeline.length,
      maxTimelineErrorUs,
      audioPackets: audioStats.packetCount,
      audioPeak,
    };
  } finally {
    input.dispose();
  }
}

class MediabunnyOutputAdapter {
  static async create(profile, audioConfig, hardwareAcceleration, telemetry, outputMode) {
    const storage = await createOutputStorage(outputMode);
    if (storage.kind === OutputStorageKind.OPFS) {
      try {
      const { handle } = storage;
      const fileStream = await handle.createWritable();
      const metrics = { writes: 0, maxWriteBytes: 0 };
      const writable = new WritableStream({
        async write(chunk) {
          metrics.writes += 1;
          metrics.maxWriteBytes = Math.max(metrics.maxWriteBytes, chunk.data.byteLength);
          await fileStream.write(chunk);
        },
        close: () => fileStream.close(),
        abort: reason => fileStream.abort(reason),
      });
      storage.target = new StreamTarget(writable, { chunked: true, chunkSize: OUTPUT_CHUNK_BYTES });
      storage.metrics = metrics;
      } catch (error) {
        try { await storage.root.removeEntry(storage.name); } catch (_) { /* partial file */ }
        Object.assign(storage, { kind: OutputStorageKind.MEMORY, root: null, handle: null,
          name: null, fallbackReason: errorMessage(error) });
      }
    }
    if (storage.kind === OutputStorageKind.MEMORY) storage.target = new BufferTarget();
    return new MediabunnyOutputAdapter(
      profile,
      audioConfig,
      hardwareAcceleration,
      telemetry,
      storage,
    );
  }

  constructor(profile, audioConfig, hardwareAcceleration, telemetry, storage) {
    this.storage = storage;
    this.target = storage.target;
    this.videoPackets = 0;
    this.audioPackets = 0;
    this.videoEncoderConfig = null;
    this.audioEncoderConfig = null;
    const format = profile.container === "mp4"
      ? new Mp4OutputFormat({ fastStart: storage.handle ? false : "in-memory" })
      : new WebMOutputFormat();
    this.output = new Output({ format, target: this.target });
    this.videoSource = new VideoSampleSource({
      codec: profile.videoCodec,
      quality: new Quality("high"),
      keyFrameInterval: 2,
      latencyMode: "quality",
      hardwareAcceleration,
      sizeChangeBehavior: "deny",
      onEncoderConfig: (config) => { this.videoEncoderConfig = config; },
      onEncodedSample: () => telemetry.enter("videoEncoderCallbacks"),
      onEncodedPacket: () => {
        this.videoPackets += 1;
        telemetry.leave("videoEncoderCallbacks");
        telemetry.pulse("videoPacketCallbacks");
      },
    });
    this.output.addVideoTrack(this.videoSource, { name: "M3 resized video" });
    this.audioSource = null;
    if (profile.audioCodec) {
      this.audioSource = new AudioSampleSource({
        codec: profile.audioCodec,
        quality: new Quality("high"),
        transform: { sampleRate: 48_000, numberOfChannels: audioConfig.numberOfChannels },
        onEncoderConfig: (config) => { this.audioEncoderConfig = config; },
        onEncodedSample: () => telemetry.enter("audioEncoderCallbacks"),
        onEncodedPacket: () => {
          this.audioPackets += 1;
          telemetry.leave("audioEncoderCallbacks");
          telemetry.pulse("audioPacketCallbacks");
        },
      });
      this.output.addAudioTrack(this.audioSource, { name: "M3 transcoded audio" });
    }
  }

  start() { return this.output.start(); }
  finalize() { return this.output.finalize(); }
  cancel() { return this.output.cancel(); }

  async finalizedBlob(profile) {
    if (this.storage.handle) {
      const file = await this.storage.handle.getFile();
      retainedOutputStorage = this.storage;
      return file;
    }
    if (!this.target.buffer) fail(`${profile.container.toUpperCase()} finalization produced no output buffer.`);
    return new Blob([this.target.buffer], { type: profile.mimeType });
  }

  async discardFile() {
    if (!this.storage.root || !this.storage.name) return;
    try { await this.storage.root.removeEntry(this.storage.name); }
    catch (_) { /* a missing or already-removed partial file needs no recovery */ }
    if (retainedOutputStorage === this.storage) retainedOutputStorage = null;
  }

  storageReport() {
    return this.storage.handle
      ? `Output storage: bounded OPFS stream; Mediabunny target chunk=${OUTPUT_CHUNK_BYTES / 1048576} MiB with WritableStream backpressure; writes=${this.storage.metrics.writes}, maximum write=${this.storage.metrics.maxWriteBytes} bytes; temporary entry retained only for the active download and removed on replacement/page exit; compressed output was not accumulated in an application ArrayBuffer.`
      : `Output storage: memory fallback (maximum input ${MEMORY_FALLBACK_MAX_INPUT_BYTES / 1048576} MiB); bounded origin-private file streaming was unavailable: ${this.storage.fallbackReason}.`;
  }
}

// The shared wgpu processor still supplies every video frame. FFmpeg's raw
// RGBA ingress is a deliberate CPU readback, spooled to OPFS one frame at a
// time rather than retained in JS or copied into the Emscripten filesystem.
class FfmpegWasmOutputAdapter {
  static async create(file, width, height, frameRate, packetCount, cancelled, status, outputMode) {
    const rate = ffmpegFrameRate(frameRate);
    if (!rate) fail(`FFmpeg WASM requires a supported constant frame rate (reported ${frameRate}).`);
    const storage = await createOutputStorage(outputMode, true);
    let rawWriter;
    try {
      storage.rawName = OPFS_RAW_NAME;
      storage.rawHandle = await storage.root.getFileHandle(storage.rawName, { create: true });
      const estimate = await navigator.storage.estimate?.();
      const rawEstimate = width * height * 4 * packetCount;
      if (estimate && Number.isFinite(estimate.quota) && Number.isFinite(estimate.usage)
          && estimate.quota - estimate.usage < rawEstimate + file.size) {
        fail("Insufficient origin-private storage quota for FFmpeg raw-frame streaming.");
      }
      rawWriter = await storage.rawHandle.createWritable();
    } catch (error) {
      try { await storage.root.removeEntry(storage.rawName); } catch (_) { /* no spool */ }
      try { await storage.root.removeEntry(storage.name); } catch (_) { /* no output */ }
      throw error;
    }
    const { FFmpeg } = await import("@ffmpeg/ffmpeg");
    const ffmpeg = new FFmpeg();
    const adapter = new FfmpegWasmOutputAdapter(ffmpeg, file, width, height, rate, cancelled, storage, rawWriter);
    ffmpeg.on("log", ({ message }) => {
      const match = /^DIAXUS_WASM_HEAP_PEAK=(\d+)$/.exec(message);
      if (match) adapter.peakWasmHeapBytes = Math.max(adapter.peakWasmHeapBytes, Number(match[1]));
      const output = /^DIAXUS_OPFS_OUTPUT=(.+)$/.exec(message);
      if (output) adapter.outputMetrics = JSON.parse(output[1]);
    });
    const started = performance.now();
    const watcher = setInterval(() => { if (cancelled()) adapter.terminate(); }, 25);
    let timeout;
    try {
      status("Loading the optional FFmpeg WASM core (32.2 MB uncompressed)…");
      await Promise.race([
        ffmpeg.load(globalThis.__DIAXUS_FFMPEG_ASSETS__),
        new Promise((_, reject) => {
          timeout = setTimeout(() => {
            adapter.terminate();
            reject(Error("FFmpeg WASM core startup exceeded 45 seconds."));
          }, 45_000);
        }),
      ]);
      adapter.startupMs = performance.now() - started;
      if (cancelled()) throw Error("CANCELLED: stopped during FFmpeg WASM startup");
      return adapter;
    } catch (error) {
      adapter.terminate();
      await adapter.discardFile();
      throw error;
    } finally {
      clearTimeout(timeout);
      clearInterval(watcher);
    }
  }

  constructor(ffmpeg, file, width, height, frameRate, cancelled, storage, rawWriter) {
    this.ffmpeg = ffmpeg;
    this.file = file;
    this.width = width;
    this.height = height;
    this.frameRate = frameRate;
    this.cancelled = cancelled;
    this.storage = storage;
    this.rawWriter = rawWriter;
    this.videoEncoderConfig = null;
    this.audioEncoderConfig = null;
    this.videoPackets = 0;
    this.audioPackets = 0;
    this.frameCount = 0;
    this.firstTimestamp = null;
    this.rawBytes = 0;
    this.readbacks = 0;
    this.audioSamples = 0;
    this.terminated = false;
    this.peakWasmHeapBytes = 0;
    this.outputMetrics = null;
    this.videoSource = { add: (frame, timestamp, duration) => this.addVideo(frame, timestamp, duration) };
    this.audioSource = { add: async () => { this.audioSamples += 1; } };
  }

  start() { return Promise.resolve(); }

  async addVideo(frame, timestamp, duration) {
    if (this.cancelled()) throw Error("CANCELLED: stopped before FFmpeg WASM frame readback");
    const index = this.frameCount;
    if (this.firstTimestamp === null) this.firstTimestamp = timestamp;
    const frameUs = 1_000_000 * this.frameRate.den / this.frameRate.num;
    const expectedTimestamp = this.firstTimestamp + Math.round(index * frameUs);
    const expectedDuration = Math.round((index + 1) * frameUs) - Math.round(index * frameUs);
    if (Math.abs(timestamp - expectedTimestamp) > 1 || Math.abs(duration - expectedDuration) > 1) {
      fail(`FFmpeg WASM raw-video bridge requires constant ${this.frameRate.num}/${this.frameRate.den} fps; frame ${index} is ${timestamp}+${duration} µs, expected ${expectedTimestamp}+${expectedDuration} µs.`);
    }
    const bytes = this.width * this.height * 4;
    if (!Number.isSafeInteger(bytes) || this.rawBytes + bytes > FFMPEG_MAX_RAW_SPOOL_BYTES) {
      fail("FFmpeg WASM exceeds its 2 GiB raw-frame disk spool limit.");
    }
    const rgba = new Uint8Array(bytes);
    await frame.copyTo(rgba, { format: "RGBA" });
    await this.rawWriter.write(rgba);
    this.rawBytes += bytes;
    this.readbacks += 1;
    this.frameCount += 1;
  }

  async finalize() {
    if (this.cancelled()) throw Error("CANCELLED: stopped before FFmpeg WASM encode");
    await this.rawWriter.close();
    this.rawWriter = null;
    await this.ffmpeg.createDir("/input");
    if (!(await this.ffmpeg.mount("WORKERFS", { blobs: [{ name: "source.mp4", data: this.file }] }, "/input"))) {
      fail("FFmpeg WASM could not mount the source File for streaming reads.");
    }
    if (!(await this.ffmpeg.mount("DIAXUS_OPFS", { fileHandle: this.storage.rawHandle, mode: "read" }, "/dev/diaxus-raw"))) {
      fail("FFmpeg WASM could not mount the raw-frame OPFS input.");
    }
    if (!(await this.ffmpeg.mount("DIAXUS_OPFS", { fileHandle: this.storage.handle, mode: "write" }, "/dev/diaxus-output"))) {
      fail("FFmpeg WASM could not mount the OPFS output.");
    }
    if (this.cancelled()) throw Error("CANCELLED: stopped before FFmpeg WASM encode");
    const args = ["-f", "rawvideo", "-pixel_format", "rgba", "-video_size", `${this.width}x${this.height}`,
      "-framerate", `${this.frameRate.num}/${this.frameRate.den}`, "-i", "/dev/diaxus-raw", "-i", "/input/source.mp4",
      "-map", "0:v:0", "-map", "1:a:0", "-c:v", "libx264", "-preset", "veryfast",
      "-crf", "20", "-pix_fmt", "yuv420p", "-vf", `settb=1/1000000,setpts=PTS+${this.firstTimestamp}`,
      "-enc_time_base:v", "1:1000000", "-fps_mode", "passthrough", "-c:a", "aac", "-b:a", "192k",
      "-ar", "48000", "-video_track_timescale", "1000000", "-movie_timescale", "1000000",
      "-f", "mp4", "-movflags", "+faststart", "/dev/diaxus-output"];
    const watcher = setInterval(() => { if (this.cancelled()) this.terminate(); }, 25);
    try {
      const code = await this.ffmpeg.exec(args);
      if (this.cancelled()) throw Error("CANCELLED: stopped during FFmpeg WASM encode");
      if (code !== 0) fail(`FFmpeg WASM encoder exited with code ${code}.`);
      this.videoPackets = this.frameCount;
      this.audioPackets = this.audioSamples > 0 ? 1 : 0;
    } finally {
      clearInterval(watcher);
      this.terminate();
    }
  }

  terminate() {
    if (!this.terminated) { this.ffmpeg.terminate(); this.terminated = true; }
  }
  cancel() { this.terminate(); }
  async discardFile() {
    this.terminate();
    if (this.rawWriter) {
      try { await this.rawWriter.abort(); } catch (_) { /* already closed */ }
      this.rawWriter = null;
    }
    for (const name of [this.storage.rawName, this.storage.name]) {
      try { await removeOpfsEntry(this.storage.root, name); } catch (_) { /* retain primary error */ }
    }
    if (retainedOutputStorage === this.storage) retainedOutputStorage = null;
  }
  async finalizedBlob() {
    const file = await this.storage.handle.getFile();
    if (!file.size) fail("FFmpeg WASM produced no output file.");
    retainedOutputStorage = this.storage;
    await removeOpfsEntry(this.storage.root, this.storage.rawName);
    return file;
  }
  storageReport() {
    return `FFmpeg WASM: startup=${this.startupMs.toFixed(1)} ms; raw RGBA spooled to OPFS=${this.rawBytes} bytes in ${this.readbacks} frame writes; source File mounted via WORKERFS (no whole-file application copy); output OPFS device=${JSON.stringify(this.outputMetrics)}; explicit GPU→CPU VideoFrame.copyTo readbacks=${this.readbacks}; sampled peak allocated WASM linear memory=${this.peakWasmHeapBytes} bytes (25 ms sampling); total process resident peak not observable; raw disk limit=${FFMPEG_MAX_RAW_SPOOL_BYTES} bytes; WASM worker terminated=${this.terminated}.`;
  }
}

async function runBrowserJob(file, outputWidth, outputHeight, profileId, requestedAcceleration, outputMode, verifyOutputFully, failureMode, processFrame, bitmapIngressRequired, status, cancelled) {
  if (!globalThis.isSecureContext || !globalThis.VideoDecoder || !globalThis.VideoEncoder || !navigator.gpu) {
    fail("The browser backend requires a secure context, WebCodecs, and WebGPU.");
  }

  const profile = resolveProfile(profileId);
  const ffmpeg = ffmpegSelected();
  const requested = requestedAcceleration === "prefer-hardware" ? "prefer-hardware" : "no-preference";
  const opened = await MediabunnyInputAdapter.open(file);
  let muxer = null;
  let outputStarted = false;
  let completed = false;
  let processed = 0;
  let audioSamples = 0;
  const stages = Object.create(null);
  const telemetry = {
    enter(name) {
      const stage = stages[name] ??= { live: 0, peak: 0, total: 0 };
      stage.live += 1;
      stage.total += 1;
      stage.peak = Math.max(stage.peak, stage.live);
    },
    leave(name) {
      const stage = stages[name];
      if (stage) stage.live = Math.max(0, stage.live - 1);
    },
    pulse(name) { this.enter(name); this.leave(name); },
    settle(name) {
      const stage = stages[name];
      if (stage) {
        stage.discarded = (stage.discarded ?? 0) + stage.live;
        stage.live = 0;
      }
    },
    report() {
      const value = name => stages[name] ?? { live: 0, peak: 0, total: 0 };
      return `Stage telemetry (live/peak/total/discarded): decoded-video ${value("decodedVideo").live}/${value("decodedVideo").peak}/${value("decodedVideo").total}/${value("decodedVideo").discarded ?? 0}; GPU ${value("gpu").live}/${value("gpu").peak}/${value("gpu").total}/${value("gpu").discarded ?? 0}; video encoder callbacks ${value("videoEncoderCallbacks").live}/${value("videoEncoderCallbacks").peak}/${value("videoEncoderCallbacks").total}/${value("videoEncoderCallbacks").discarded ?? 0}; video packet callbacks ${value("videoPacketCallbacks").live}/${value("videoPacketCallbacks").peak}/${value("videoPacketCallbacks").total}/${value("videoPacketCallbacks").discarded ?? 0}; decoded-audio ${value("decodedAudio").live}/${value("decodedAudio").peak}/${value("decodedAudio").total}/${value("decodedAudio").discarded ?? 0}; audio encoder callbacks ${value("audioEncoderCallbacks").live}/${value("audioEncoderCallbacks").peak}/${value("audioEncoderCallbacks").total}/${value("audioEncoderCallbacks").discarded ?? 0}; audio packet callbacks ${value("audioPacketCallbacks").live}/${value("audioPacketCallbacks").peak}/${value("audioPacketCallbacks").total}/${value("audioPacketCallbacks").discarded ?? 0}.`;
    },
  };
  // Application-held references only; codec/library-internal resources are opaque.
  const retained = { frames: 0, samples: 0, peakFrames: 0, peakSamples: 0 };
  const retain = kind => {
    retained[kind]++;
    const peak = kind === "frames" ? "peakFrames" : "peakSamples";
    retained[peak] = Math.max(retained[peak], retained[kind]);
  };
  const release = (resource, kind) => {
    if (resource) { resource.close(); retained[kind]--; }
  };
  const lifecycle = () => `Cleanup: ${retained.frames} application-held frame references, ${retained.samples} samples; peaks ${retained.peakFrames}/${retained.peakSamples} (library-internal resources not counted).`;
  const bitmapPreparation = { live: 0, peak: 0, total: 0, groups: 0, cumulativeMs: 0, consumerWaitMs: 0 };
  const bitmapPreparationReport = () => `Bitmap preparation: total=${bitmapPreparation.total}; groups=${bitmapPreparation.groups}; live/peak=${bitmapPreparation.live}/${bitmapPreparation.peak}; cumulative task latency=${bitmapPreparation.cumulativeMs.toFixed(1)} ms; ordered-consumer wait=${bitmapPreparation.consumerWaitMs.toFixed(1)} ms; bound=4.`;
  const prepareBitmap = frame => {
    bitmapPreparation.live++;
    bitmapPreparation.total++;
    bitmapPreparation.peak = Math.max(bitmapPreparation.peak, bitmapPreparation.live);
    const started = performance.now();
    return createImageBitmap(frame).finally(() => {
      bitmapPreparation.cumulativeMs += performance.now() - started;
      bitmapPreparation.live--;
    });
  };
  const startedAt = performance.now();
  try {
    if (opened.displayWidth < outputWidth || opened.displayHeight < outputHeight) {
      fail("The half-size preset must not upscale the source.");
    }
    let selectedAcceleration = requested;
    let accelerationFallback = null;
    let profileCapability = await probeProfile(profile, opened, outputWidth, outputHeight, selectedAcceleration);
    if (!profileCapability.supported && requested === "prefer-hardware") {
      const preferredReason = profileCapability.reason;
      profileCapability = await probeProfile(profile, opened, outputWidth, outputHeight, "no-preference");
      if (profileCapability.supported) {
        selectedAcceleration = "no-preference";
        accelerationFallback = preferredReason;
        status(`Hardware-preferred codec configuration is unavailable; using the compatibility baseline: ${preferredReason}`);
      }
    }
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
    status(`Demuxed MP4/${opened.codec === "hevc" ? "H.265/HEVC" : "H.264/AVC"}: coded ${opened.codedWidth}×${opened.codedHeight}, visible ${opened.visibleRect.width}×${opened.visibleRect.height}+${opened.visibleRect.left},${opened.visibleRect.top}, square-pixel ${opened.width}×${opened.height}, rotation ${opened.rotation}°, flip=${opened.flip}, display ${opened.displayWidth}×${opened.displayHeight}, PAR ${opened.pixelAspectRatio.num}:${opened.pixelAspectRatio.den}; ${opened.packetCount} video packets; profile ${profile.label}; codec preference ${selectedAcceleration}.`);

    muxer = ffmpeg
      ? await FfmpegWasmOutputAdapter.create(file, outputWidth, outputHeight, opened.averagePacketRate, opened.packetCount, cancelled, status, outputMode)
      : await MediabunnyOutputAdapter.create(profile, opened.audio, selectedAcceleration, telemetry, outputMode);
    if (!muxer.storage.handle && file.size > MEMORY_FALLBACK_MAX_INPUT_BYTES) {
      fail(`The selected file is ${(file.size / 1048576).toFixed(1)} MiB. Bounded output streaming is unavailable (${muxer.storage.fallbackReason}); the memory fallback accepts at most ${MEMORY_FALLBACK_MAX_INPUT_BYTES / 1048576} MiB inputs.`);
    }
    status(muxer.storageReport());
    try {
      await muxer.start();
    } catch (error) {
      fail(`${profile.container.toUpperCase()} muxer initialization failed: ${errorMessage(error)}`);
    }
    outputStarted = true;

    let videoEndSeconds = 0;
    let audioEndSeconds = 0;
    let lastVideoDurationSeconds = 0;
    const videoTimeline = [];
    let siblingFailure = null;
    const ensureActive = () => {
      if (siblingFailure) throw siblingFailure;
      if (cancelled()) throw new Error("CANCELLED: conversion stopped by user");
    };
    const pumpVideo = async () => {
      const sink = new VideoSampleSink(opened.track, { hardwareAcceleration: selectedAcceleration });
      const iterator = sink.samples()[Symbol.asyncIterator]();
      let previousTimestamp = null;
      const createItem = (sample, prepareInParallel) => {
        telemetry.enter("decodedVideo");
        retain("samples");
        let decodedFrame = null;
        try {
          ensureActive();
          const sourceTimestamp = asSafeMicroseconds(sample.microsecondTimestamp, "input video timestamp");
          const reportedDuration = asSafeMicroseconds(sample.microsecondDuration, "input video duration");
          const duration = reportedDuration > 0 ? reportedDuration : Math.round(1_000_000 / opened.averagePacketRate);
          const timestamp = sourceTimestamp - originUs;
          if (previousTimestamp !== null && timestamp <= previousTimestamp) {
            fail(`Video timestamps are not strictly increasing: ${timestamp} µs followed ${previousTimestamp} µs.`);
          }
          previousTimestamp = timestamp;
          videoTimeline.push({ timestamp, duration });
          const visible = sample.visibleRect;
          if (sample.codedWidth !== opened.codedWidth || sample.codedHeight !== opened.codedHeight
              || visible.left !== opened.visibleRect.left || visible.top !== opened.visibleRect.top
              || visible.width !== opened.visibleRect.width || visible.height !== opened.visibleRect.height
              || sample.squarePixelWidth !== opened.width || sample.squarePixelHeight !== opened.height
              || sample.rotation !== opened.rotation || sample.flip !== opened.flip) {
            fail(`Mid-stream geometry change is unsupported: frame ${sample.codedWidth}×${sample.codedHeight}, visible ${visible.width}×${visible.height}+${visible.left},${visible.top}, square-pixel ${sample.squarePixelWidth}×${sample.squarePixelHeight}, rotation ${sample.rotation}°, flip=${sample.flip}.`);
          }
          const sampleColor = validateSdrColorSpace(sample.colorSpace, false);
          if (sampleColor.primaries !== opened.color.primaries
              || sampleColor.transfer !== opened.color.transfer
              || sampleColor.matrix !== opened.color.matrix
              || sampleColor.fullRange !== opened.color.fullRange) {
            fail("Mid-stream color metadata change is unsupported.");
          }
          decodedFrame = sample.toVideoFrame();
          if (decodedFrame.displayWidth !== opened.width || decodedFrame.displayHeight !== opened.height) {
            fail(`Decoded VideoFrame display geometry ${decodedFrame.displayWidth}×${decodedFrame.displayHeight} does not match normalized square-pixel input ${opened.width}×${opened.height}.`);
          }
          retain("frames");
          return {
            sample,
            decodedFrame,
            timestamp,
            duration,
            bitmapPromise: prepareInParallel ? prepareBitmap(decodedFrame) : Promise.resolve(null),
            bitmapTransferred: false,
          };
        } catch (error) {
          release(sample, "samples");
          release(decodedFrame, "frames");
          telemetry.leave("decodedVideo");
          throw error;
        }
      };
      const discardItem = async item => {
        if (!item.bitmapTransferred) {
          try { (await item.bitmapPromise)?.close(); } catch (_) { /* retain original failure */ }
        }
        release(item.sample, "samples");
        release(item.decodedFrame, "frames");
        telemetry.leave("decodedVideo");
      };
      const processItem = async item => {
        let processedFrame = null;
        let bitmap = null;
        try {
          ensureActive();
          const bitmapWaitStarted = performance.now();
          bitmap = await item.bitmapPromise;
          bitmapPreparation.consumerWaitMs += performance.now() - bitmapWaitStarted;
          ensureActive();
          // Calling into Rust transfers ownership of a prepared bitmap even if
          // the returned promise later rejects. Rust closes it on every path.
          item.bitmapTransferred = bitmap !== null;
          telemetry.enter("gpu");
          try {
            processedFrame = await processFrame(
              item.decodedFrame,
              bitmap,
              item.timestamp,
              item.duration,
            );
          } finally {
            telemetry.leave("gpu");
          }
          retain("frames");
          const outputSample = ffmpeg ? null : new VideoSample(processedFrame);
          if (outputSample) retain("samples");
          try {
            if (failureMode === "codec-once"
                && !consumedFailureInjections.has(failureMode)
                && processed === 4) {
              consumedFailureInjections.add(failureMode);
              fail("INJECTED: codec failure after 5 GPU-processed frames");
            }
            await muxer.videoSource.add(outputSample ?? processedFrame, item.timestamp, item.duration);
          } catch (error) {
            if (errorMessage(error).startsWith("INJECTED:")) throw error;
            const config = muxer.videoEncoderConfig;
            const configured = config
              ? `${config.codec}, ${config.width}×${config.height}, ${config.bitrate ?? "auto"} bit/s, hardware=${config.hardwareAcceleration ?? "no-preference"}`
              : `${outputWidth}×${outputHeight} ${profile.videoCodec.toUpperCase()} (encoder config was not emitted)`;
            fail(`${profile.videoCodec.toUpperCase()} encoder configuration failed (${configured}): ${errorMessage(error)}`);
          } finally {
            release(outputSample, "samples");
          }
          processed += 1;
          lastVideoDurationSeconds = item.duration / 1_000_000;
          videoEndSeconds = (item.timestamp + item.duration) / 1_000_000;
          const percent = opened.packetCount > 0 ? Math.min(99, Math.floor((processed / opened.packetCount) * 100)) : 0;
          status(`Converting ${percent}% — video ${processed}/${opened.packetCount || "?"}, audio ${audioSamples}/${opened.audio?.packetCount ?? 0}.`);
        } finally {
          if (bitmap && !item.bitmapTransferred) bitmap.close();
          release(processedFrame, "frames");
          release(item.sample, "samples");
          release(item.decodedFrame, "frames");
          telemetry.leave("decodedVideo");
        }
      };
      try {
        const first = await iterator.next();
        if (first.done) return;
        await processItem(createItem(first.value, false));

        if (!bitmapIngressRequired()) {
          for (;;) {
            ensureActive();
            const next = await iterator.next();
            if (next.done) break;
            await processItem(createItem(next.value, false));
          }
          return;
        }

        for (;;) {
          const group = [];
          try {
            for (let index = 0; index < 4; index++) {
              ensureActive();
              const next = await iterator.next();
              if (next.done) break;
              group.push(createItem(next.value, true));
            }
          } catch (error) {
            await Promise.allSettled(group.map(discardItem));
            throw error;
          }
          if (group.length === 0) break;
          bitmapPreparation.groups++;
          for (let index = 0; index < group.length; index++) {
            try {
              await processItem(group[index]);
            } catch (error) {
              await Promise.allSettled(group.slice(index + 1).map(discardItem));
              throw error;
            }
          }
          if (group.length < 4) break;
        }
      } finally {
        try { await iterator.return?.(); } catch (_) { /* retain pipeline result */ }
      }
    };

    const pumpAudio = async () => {
      if (!muxer.audioSource) return;
      const sink = new AudioSampleSink(opened.audio.track);
      for await (const sample of sink.samples()) {
        telemetry.enter("decodedAudio");
        retain("samples");
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
          release(sample, "samples");
          telemetry.leave("decodedAudio");
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
    const outputBlob = await muxer.finalizedBlob(profile);
    if (muxer.videoPackets !== processed) {
      fail(`Video encoder emitted ${muxer.videoPackets} packets for ${processed} processed frames.`);
    }
    if (profile.audioCodec && muxer.audioPackets === 0) {
      fail(`${profile.audioCodec.toUpperCase()} encoder emitted no packets.`);
    }

    const conversionElapsed = performance.now() - startedAt;
    let verified;
    let verificationElapsed = 0;
    if (verifyOutputFully || ffmpeg) {
      status(`Inspecting finalized ${profile.container.toUpperCase()} and decoding video/audio at beginning, midpoint, and end…`);
      const verificationStarted = performance.now();
      try {
        verified = await verifyOutput(outputBlob, {
          container: profile.container,
          mimeType: profile.mimeType,
          videoCodec: profile.videoCodec,
          videoCodecLabel: profile.videoCodec === "avc" ? "H.264" : profile.videoCodec === "hevc" ? "H.265/HEVC" : "VP8",
          audioCodec: profile.audioCodec,
          audioCodecLabel: profile.audioCodec === "aac" ? "AAC" : "Opus",
          width: outputWidth,
          height: outputHeight,
          frameCount: processed,
          videoTimeline,
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
      verificationElapsed = performance.now() - verificationStarted;
    } else {
      verified = {
        duration: Math.max(videoEndSeconds, profile.audioCodec ? audioEndSeconds : 0),
        audioPackets: muxer.audioPackets,
      };
    }

    const base = file.name.replace(/\.[^.]+$/, "") || "converted";
    const totalElapsed = performance.now() - startedAt;
    const verificationSummary = (verifyOutputFully || ffmpeg)
      ? `Diagnostic verification: full re-decode PASS in ${verificationElapsed.toFixed(1)} ms; ${verified.timelinePackets}-frame timestamp/duration timeline PASS (maximum container quantization ${verified.maxTimelineErrorUs} µs)${profile.audioCodec ? `; decoded output audio peak=${verified.audioPeak.toFixed(4)}` : ""}; total job ${totalElapsed.toFixed(1)} ms.`
      : `Diagnostic verification: skipped for normal conversion; use ?verify=full for the test-only re-decode path; total job ${totalElapsed.toFixed(1)} ms.`;
    const durationSummary = (verifyOutputFully || ffmpeg)
      ? `${verified.duration.toFixed(3)} s`
      : `expected timeline ${verified.duration.toFixed(3)} s (output not re-decoded)`;
    const inputCodecLabel = opened.codec === "hevc" ? "H.265/HEVC" : "H.264";
    return {
      summary: (profile.audioCodec
        ? `PASS: ${processed} ${inputCodecLabel} input frames + ${audioSamples} decoded audio samples → ${outputWidth}×${outputHeight} ${profile.label} converted/finalized in ${conversionElapsed.toFixed(1)} ms; ${durationSummary}; ${verified.audioPackets} ${profile.audioCodec.toUpperCase()} packets; audio queue peak=1; conversion pixel readbacks=${ffmpeg ? muxer.readbacks : 0}; backend=${ffmpeg ? "FFmpeg WASM" : "WebCodecs"}.`
        : `PASS: ${processed} ${inputCodecLabel} input frames → ${outputWidth}×${outputHeight} ${profile.label} converted/finalized in ${conversionElapsed.toFixed(1)} ms; ${durationSummary}; conversion pixel readbacks=0; backend=WebCodecs.`)
        + `\n${verificationSummary}`
        + `\nCodec acceleration: requested=${requested}, selected=${selectedAcceleration}; ${ffmpeg ? "browser decoder probed, FFmpeg software encoder selected explicitly" : "exact decoder+encoder probes passed"}${accelerationFallback ? ` after visible fallback (${accelerationFallback})` : ""}; hardware execution unknown.`
        + `\nGeometry/color: input coded ${opened.codedWidth}×${opened.codedHeight}, visible ${opened.visibleRect.width}×${opened.visibleRect.height}+${opened.visibleRect.left},${opened.visibleRect.top}, PAR ${opened.pixelAspectRatio.num}:${opened.pixelAspectRatio.den}, rotation ${opened.rotation}°, flip=${opened.flip}; baked square-pixel output ${outputWidth}×${outputHeight}; SDR ${opened.color.primaries ?? "unspecified"}/${opened.color.transfer ?? "unspecified"}/${opened.color.matrix ?? "unspecified"}, browser-normalized to sRGB processing; HDR rejected.`
        + `\nBounds: input BlobSource cache ≤${INPUT_CACHE_BYTES / 1048576} MiB; Mediabunny decoder combined packet/callback queue ≤40 before output and ≤8 with decoded samples; bitmap preparation and retained GPU submissions are each ≤4; prepared frames are consumed in timestamp order; ${ffmpeg ? "raw RGBA OPFS spool ≤2 GiB; FFmpeg WASM internal memory is not otherwise bounded" : "WebCodecs encoder queue ≤4; mux writes are serialized"}.`
        + `\n${muxer.storageReport()}`
        + `\n${bitmapPreparationReport()}\n${telemetry.report()}\n${lifecycle()}`,
      // Only a Blob/File handle crosses the execution-context boundary. OPFS-backed
      // output remains disk-backed; the host owns the object URL lifecycle.
      blob: outputBlob,
      fileName: `${base}-${outputWidth}x${outputHeight}.${profile.extension}`,
      frameCount: processed,
      duration: verified.duration,
      outputBytes: outputBlob.size,
    };
  } catch (error) {
    if (outputStarted && !completed) {
      try { await muxer.cancel(); } catch (_) { /* retain original failure */ }
      // A successful cancel closes the library-owned encoders. Accepted samples
      // without output callbacks are accounted as discarded, not left live.
      telemetry.settle("videoEncoderCallbacks");
      telemetry.settle("audioEncoderCallbacks");
    }
    await muxer?.discardFile();
    const message = errorMessage(error);
    if (isEmbeddedElectronBrowser() && !message.includes("VS Code embedded browser")) {
      throw new Error(`${message}${environmentHint()}\n${bitmapPreparationReport()}\n${lifecycle()}`);
    }
    throw new Error(`${message}\n${bitmapPreparationReport()}\n${telemetry.report()}\n${lifecycle()}`, { cause: error });
  } finally {
    opened.input.dispose();
  }
}

class WebCodecsMediabunnyBackend {
  cleanupOutput() {
    return cleanupRetainedOutput();
  }

  inspect(file) {
    return inspect(file);
  }

  probeProfiles(file, outputWidth, outputHeight) {
    return probeProfiles(file, outputWidth, outputHeight);
  }

  run(file, outputWidth, outputHeight, profileId, acceleration, outputMode, verifyOutputFully, failureMode, processFrame, bitmapIngressRequired, status, cancelled) {
    return runBrowserJob(file, outputWidth, outputHeight, profileId, acceleration, outputMode, verifyOutputFully, failureMode, processFrame, bitmapIngressRequired, status, cancelled);
  }
}

globalThis.__DIAXUS_MEDIA_WEB__ = new WebCodecsMediabunnyBackend();
