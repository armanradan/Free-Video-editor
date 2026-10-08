import assert from "node:assert/strict";
import test from "node:test";
import * as clock from "../crates/media-web/src/preview-clock.js";

class FakeAudio {
  static instances = [];
  constructor() {
    this.currentTime = 0; this.paused = true; this.ended = false;
    this.readyState = 2; this.seeking = false;
    FakeAudio.instances.push(this);
  }
  play() { this.paused = false; return this.pending ?? (this.reject ? Promise.reject(Error("codec unavailable")) : Promise.resolve()); }
  pause() { this.paused = true; }
  removeAttribute() { this.src = ""; }
  load() { this.loaded = true; }
}

test("preview clock retains one source, maps its origin, pauses, seeks and restarts at EOF", async () => {
  globalThis.Audio = FakeAudio;
  const create = URL.createObjectURL, revoke = URL.revokeObjectURL;
  let created = 0, released = 0;
  URL.createObjectURL = () => `blob:test-${++created}`;
  URL.revokeObjectURL = () => { released++; };
  try {
    const file = {};
    const play = clock.startPreviewClock(file, 2, 5, 10, false);
    const audio = FakeAudio.instances.at(-1);
    assert.equal(audio.paused, false, "play must execute before awaiting worker work");
    await play;
    assert.equal(audio.currentTime, 7);
    assert.equal(clock.previewClockPosition(), 2);
    assert.equal(clock.previewClockPlaying(), true);
    clock.seekPreviewClock(4);
    assert.equal(audio.currentTime, 9);
    audio.seeking = true;
    assert.equal(clock.previewClockReady(), false);
    audio.seeking = false;
    clock.pausePreviewClock();
    assert.equal(clock.previewClockPlaying(), false);
    await clock.startPreviewClock(file, 4, 5, 10, true);
    assert.equal(created, 1);
    assert.equal(audio.muted, true);
    audio.currentTime = 15;
    assert.equal(clock.previewClockPlaying(), false);
    assert.equal(audio.paused, true);
    await clock.startPreviewClock(file, 10, 5, 10, false);
    assert.equal(audio.currentTime, 5);
    clock.mutePreviewClock(true);
    assert.equal(audio.muted, true);
    const replacementSource = {};
    await clock.startPreviewClock(replacementSource, 0, 0, 3, false);
    assert.equal(created, 2);
    assert.equal(released, 1);
    assert.equal(audio.src, "");
    assert.equal(audio.loaded, true);
    const replacement = FakeAudio.instances.at(-1);
    replacement.reject = true;
    await assert.rejects(clock.startPreviewClock(replacementSource, 0, 0, 3, false), /codec unavailable/);
    assert.match(clock.previewClockError(), /codec unavailable/);
    assert.equal(clock.previewClockPlaying(), false);
    replacement.reject = false;
    let rejectOld;
    replacement.pending = new Promise((_, reject) => { rejectOld = reject; });
    const oldPlay = clock.startPreviewClock(replacementSource, 0, 0, 3, false);
    await clock.startPreviewClock({}, 0, 0, 4, false);
    rejectOld(Error("retired source"));
    await assert.rejects(oldPlay, /retired source/);
    assert.equal(clock.previewClockError(), "", "retired source error affected the replacement");
    assert.equal(clock.previewClockPlaying(), true, "retired source rejection paused replacement audio");
  } finally {
    clock.releasePreviewClock();
    URL.createObjectURL = create; URL.revokeObjectURL = revoke;
    delete globalThis.Audio;
  }
});
