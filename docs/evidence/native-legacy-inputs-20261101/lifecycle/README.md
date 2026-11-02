# Integrated compatibility ownership/lifecycle (2026-11-02)

- Active inspection cancellation: the original6941-frame MPG inspection returns
  cancellation in341ms after a100ms signal, publishes no cache entry and leaves
  no matching ffprobe child. A fresh cache retry obtains all6941frames, with
  unchanged source stamp. The separate ignored test is
  `inspection::tests::actual_cancelled_inspection_does_not_publish_cache_or_leave_children`.
- Active Windows WMA preparation cancellation: the full private WMA Pro source
  is cancelled only after more than1MiB of PCM intermediate is observed. Owned
  scratch is removed. A real helper failure on a non-WMA source also removes
  scratch. A fresh full-source retry verifies8,007,825samples/2402blocks using
  48,274,724bytes, retires scratch and leaves no owned decoder.exe process.
  Source stamp is unchanged. Test:
  `legacy_audio::tests::actual_active_preparation_cancel_failure_cleanup_and_retry`.
- Preview-to-conversion handoff: a muted ordinary playback worker is cancelled
  after presenting its first frame. Inspection, preview planning and direct
  export share the exact same source-bound WMA owner. Replacing that source
  invalidates its stamp/cache and prepares a different owner; delayed consumers
  keep only the old owner alive until their final retirement. Both old/new
  scratch directories disappear after their final consumers. Test:
  `legacy_audio::tests::preview_cancel_and_conversion_share_owner_across_source_replacement`.
- Active hybrid export failure/cancel/fresh retry and zero remaining owned
  FFmpeg children pass on both NVIDIA route families; see `../hybrid/lifecycle.json`.
  Encoder preflight failure happens before partial creation, active cancellation
  is observed only after a nonempty encoder partial, and retries identify NVENC
  with correct actual software-decoder transfer counts.
- AAC source-bound preparation, disk admission, failure cleanup, delayed-owner
  retirement and source replacement remain covered by the native tests and
  twenty repeated independent route/control exports in hybrid evidence.

All paths above exercise owned native resources/processes; originals remain
read-only. PCM marker tests independently establish seek/offset/gap/tail audio
command behavior. Muted playback ownership checks do not claim speaker/device
audibility or new ordinary-window WMA interaction; that remains task6.1.

The native window could be enumerated but captured black; activation failed.
The documented recovery attempt returned `GetCursorPos failed: Access is denied.
(0x80070005)`. UI input stopped and the user was asked to restore desktop access.
No desktop/security setting was changed and no alternate UI automation was used.
