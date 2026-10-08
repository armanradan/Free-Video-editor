// Window-owned audio/clock only. Decoded images stay in the media execution
// context; no native video element or CPU canvas duplicates the GPU transform.
let audio;
let source;
let url;
let origin = 0;
let length = 0;
let failure = "";

export function releasePreviewClock() {
  if (audio) {
    audio.pause();
    audio.onerror = null;
    audio.removeAttribute("src");
    audio.load();
  }
  if (url) URL.revokeObjectURL(url);
  audio = source = url = null;
  failure = "";
}

export function startPreviewClock(file, seconds, start, duration, muted) {
  if (source !== file) {
    releasePreviewClock();
    source = file;
    audio = new Audio();
    audio.preload = "metadata";
    url = URL.createObjectURL(file);
    audio.src = url;
    const owner = audio;
    audio.onerror = () => { if (audio === owner) failure = "The browser could not play this source's audio/clock. Preview playback requires native MP4 playback support."; };
  }
  origin = Math.max(0, start);
  length = duration;
  failure = "";
  audio.muted = muted;
  seekPreviewClock(seconds >= length - .001 ? 0 : seconds);
  // Called directly from the user gesture, before any worker initialization.
  const owner = audio;
  return owner.play().catch(error => {
    owner.pause();
    if (audio === owner) failure = `Preview playback unavailable: ${error.message}`;
    throw error;
  });
}

export function pausePreviewClock() { audio?.pause(); }
export function seekPreviewClock(seconds) {
  if (audio) audio.currentTime = origin + Math.max(0, Math.min(seconds, length));
}
export function mutePreviewClock(muted) { if (audio) audio.muted = muted; }
export function previewClockPosition() {
  return Math.max(0, Math.min((audio?.currentTime ?? origin) - origin, length));
}
export function previewClockPlaying() {
  if (!audio || audio.ended || previewClockPosition() >= length) {
    audio?.pause();
    return false;
  }
  return !audio.paused;
}
export function previewClockReady() { return Boolean(audio && !audio.seeking && audio.readyState >= 2); }
export function previewClockError() { return failure; }

if (typeof window !== "undefined") window.addEventListener("pagehide", releasePreviewClock);
