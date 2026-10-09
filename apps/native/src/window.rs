//! Thin native event-loop adapter for logical viewport measurements.
//! Dioxus Native 0.7.10's element_coordinates/get_client_rect are unimplemented.
use blitz_shell::{BlitzApplication, BlitzShellEvent, View, WindowConfig};
use dioxus::native::{DioxusDocument, DioxusNativeWindowRenderer, DocumentConfig};
use dioxus::prelude::*;
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, StartCause, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{Key, ModifiersState, NamedKey},
    window::{Window, WindowAttributes, WindowId},
};

static LOGICAL_WIDTH: AtomicU64 = AtomicU64::new(960.0_f64.to_bits());
static LOGICAL_HEIGHT: AtomicU64 = AtomicU64::new(760.0_f64.to_bits());
static PREVIEW_FULLSCREEN: AtomicBool = AtomicBool::new(false);
static APP_WINDOW: Mutex<Option<Arc<Window>>> = Mutex::new(None);
static MAIN_RENDERER_LOST: AtomicBool = AtomicBool::new(false);
static PREVIEW_RENDERER_LOST: AtomicBool = AtomicBool::new(false);

#[cfg(not(test))]
pub fn report_renderer_loss(detached: bool) {
    if detached {
        &PREVIEW_RENDERER_LOST
    } else {
        &MAIN_RENDERER_LOST
    }
    .store(true, Ordering::Release);
    if let Some(window) = APP_WINDOW.lock().ok().and_then(|w| w.clone()) {
        window.request_redraw();
    }
}
static PREVIEW_WIDTH: AtomicU64 = AtomicU64::new(960.0_f64.to_bits());
static PREVIEW_HEIGHT: AtomicU64 = AtomicU64::new(760.0_f64.to_bits());

#[derive(Clone, Default, PartialEq)]
pub struct PreviewWindowState {
    pub path: String,
    pub dark: bool,
    pub before: bool,
    pub source: bool,
    pub busy: bool,
}
static PREVIEW_SETTINGS: Mutex<Option<PreviewWindowState>> = Mutex::new(None);
static BEFORE_REQUEST: Mutex<Option<(String, bool)>> = Mutex::new(None);
pub fn set_preview_settings(settings: PreviewWindowState) {
    *PREVIEW_SETTINGS.lock().unwrap() = Some(settings);
}
pub fn preview_settings() -> PreviewWindowState {
    PREVIEW_SETTINGS.lock().unwrap().clone().unwrap_or_default()
}
pub fn request_before(value: bool) {
    *BEFORE_REQUEST.lock().unwrap() = Some((preview_settings().path, value));
}
pub fn take_before_request() -> Option<(String, bool)> {
    BEFORE_REQUEST.lock().unwrap().take()
}
pub fn preview_window_open() -> bool {
    PREVIEW_FULLSCREEN.load(Ordering::Acquire)
}

pub fn set_preview_fullscreen(enabled: bool) {
    PREVIEW_FULLSCREEN.store(enabled, Ordering::Release);
    if let Ok(window) = APP_WINDOW.lock()
        && let Some(window) = window.as_ref()
    {
        // Wake the event loop; only it may create/drop native windows.
        window.request_redraw();
    }
}

pub fn preview_extent() -> (f64, f64) {
    let width = f64::from_bits(PREVIEW_WIDTH.load(Ordering::Acquire));
    let height = f64::from_bits(PREVIEW_HEIGHT.load(Ordering::Acquire));
    fit_preview(width, height)
}
fn fit_preview(width: f64, height: f64) -> (f64, f64) {
    let scale = ((width - 48.0).max(1.0) / 640.0).min((height - 210.0).max(1.0) / 360.0);
    (640.0 * scale, 360.0 * scale)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerKey {
    Toggle,
    Back,
    Forward,
    Start,
    End,
    Close,
}

// Commands only: bounded, no pixels, codec handles or Dioxus state here.
#[derive(Default)]
struct PlayerKeys {
    open: bool,
    pending: VecDeque<PlayerKey>,
}
static PLAYER_KEYS: Mutex<PlayerKeys> = Mutex::new(PlayerKeys {
    open: false,
    pending: VecDeque::new(),
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorKey {
    Step(i16),
    Min,
    Max,
}
#[derive(Default)]
struct ColorKeys {
    focused: Option<usize>,
    pending: VecDeque<(usize, ColorKey)>,
}
static COLOR_KEYS: Mutex<ColorKeys> = Mutex::new(ColorKeys {
    focused: None,
    pending: VecDeque::new(),
});

pub fn set_color_focus(index: Option<usize>) {
    if let Ok(mut keys) = COLOR_KEYS.lock() {
        if keys.focused != index {
            keys.pending.clear();
        }
        keys.focused = index;
    }
}
pub fn take_color_key() -> Option<(usize, ColorKey)> {
    COLOR_KEYS.lock().ok()?.pending.pop_front()
}
fn color_key(key: &Key, modifiers: ModifiersState) -> Option<ColorKey> {
    if modifiers.intersects(ModifiersState::CONTROL | ModifiersState::ALT | ModifiersState::SUPER) {
        return None;
    }
    Some(match key {
        Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowDown) => ColorKey::Step(-1),
        Key::Named(NamedKey::ArrowRight | NamedKey::ArrowUp) => ColorKey::Step(1),
        Key::Named(NamedKey::Home) => ColorKey::Min,
        Key::Named(NamedKey::End) => ColorKey::Max,
        _ => return None,
    })
}

impl PlayerKeys {
    fn set_open(&mut self, open: bool) {
        if self.open != open {
            self.pending.clear();
        }
        self.open = open;
    }
    fn push(&mut self, key: PlayerKey, repeat: bool) -> bool {
        if !self.open {
            return false;
        }
        if repeat && matches!(key, PlayerKey::Toggle | PlayerKey::Close) {
            return true;
        }
        if key == PlayerKey::Close {
            self.pending.clear();
        }
        if self.pending.len() < 16 {
            self.pending.push_back(key);
        }
        true
    }
}

pub fn set_player_open(open: bool) {
    if let Ok(mut keys) = PLAYER_KEYS.lock() {
        keys.set_open(open);
    }
}
pub fn take_player_key() -> Option<PlayerKey> {
    PLAYER_KEYS.lock().ok()?.pending.pop_front()
}
fn player_key(key: &Key, modifiers: ModifiersState) -> Option<PlayerKey> {
    if modifiers.intersects(ModifiersState::CONTROL | ModifiersState::ALT | ModifiersState::SUPER) {
        return None;
    }
    Some(match key {
        Key::Named(NamedKey::Space) => PlayerKey::Toggle,
        Key::Character(value) if value == " " => PlayerKey::Toggle,
        Key::Named(NamedKey::ArrowLeft) => PlayerKey::Back,
        Key::Named(NamedKey::ArrowRight) => PlayerKey::Forward,
        Key::Named(NamedKey::Home) => PlayerKey::Start,
        Key::Named(NamedKey::End) => PlayerKey::End,
        Key::Named(NamedKey::Escape) => PlayerKey::Close,
        _ => return None,
    })
}

pub fn timeline_fraction(client_x: f64) -> f64 {
    let width = f64::from_bits(LOGICAL_WIDTH.load(Ordering::Acquire));
    fraction_in_viewport(client_x, width)
}
pub fn detached_timeline_fraction(client_x: f64) -> f64 {
    fullscreen_fraction(
        client_x,
        f64::from_bits(PREVIEW_WIDTH.load(Ordering::Acquire)),
        f64::from_bits(PREVIEW_HEIGHT.load(Ordering::Acquire)),
    )
}

fn fullscreen_fraction(client_x: f64, width: f64, height: f64) -> f64 {
    let (track, _) = fit_preview(width, height);
    (client_x - (width - track) / 2.0) / track
}

// Inline left-column panel: shell 16 + panel 13 + label 112 + gap 8.
// Remaining column width follows the fixed 346px preview and 8px gutter.
pub fn color_fraction(client_x: f64) -> f64 {
    color_fraction_in_viewport(
        client_x,
        f64::from_bits(LOGICAL_WIDTH.load(Ordering::Acquire)),
    )
}

fn color_fraction_in_viewport(client_x: f64, width: f64) -> f64 {
    let shell = width.min(1100.0);
    let left = ((width - 1100.0) / 2.0).max(0.0) + 149.0;
    ((client_x - left) / (shell - 532.0).max(1.0)).clamp(0.0, 1.0)
}

fn fraction_in_viewport(client_x: f64, width: f64) -> f64 {
    // app-shell: max-width 1100, 16px padding. Right panel: 346px including
    // 12px padding + 1px border each side; its track content is exactly 320px.
    let left = ((width - 1100.0) / 2.0).max(0.0) + width.min(1100.0) - 349.0;
    (client_x - left) / 320.0
}

pub fn launch(app: fn() -> Element, attributes: WindowAttributes) {
    let event_loop = blitz_shell::create_default_event_loop::<BlitzShellEvent>();
    let document = DioxusDocument::new(VirtualDom::new(app), DocumentConfig::default());
    let config = WindowConfig::with_attributes(
        Box::new(document),
        DioxusNativeWindowRenderer::new(),
        attributes,
    );
    let inner = BlitzApplication::new(event_loop.create_proxy());
    let mut application = ViewportApplication {
        inner,
        physical_width: 960,
        physical_height: 760,
        scale: 1.0,
        initialized: false,
        modifiers: ModifiersState::empty(),
        pending: Some(config),
        main_id: None,
        preview_id: None,
        quarantined: [false; 2],
    };
    event_loop.run_app(&mut application).unwrap();
}

struct ViewportApplication {
    inner: BlitzApplication<DioxusNativeWindowRenderer>,
    pending: Option<WindowConfig<DioxusNativeWindowRenderer>>,
    physical_width: u32,
    physical_height: u32,
    scale: f64,
    initialized: bool,
    modifiers: ModifiersState,
    main_id: Option<WindowId>,
    preview_id: Option<WindowId>,
    quarantined: [bool; 2],
}

impl ViewportApplication {
    fn dispatch_window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: WindowId,
        event: WindowEvent,
    ) {
        // Loss can occur on the worker while Vello is already drawing. The
        // pinned renderer panics on that invalid texture rather than returning
        // an error. Retire it only when our device callback confirms loss;
        // unrelated panics keep their original behavior. Never resume partially
        // unwound Vello state: F5 constructs a new document/renderer/context.
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.inner.window_event(event_loop, id, event)
        }));
        if let Err(panic) = outcome {
            let confirmed = if self.preview_id == Some(id) {
                PREVIEW_RENDERER_LOST.load(Ordering::Acquire)
            } else {
                MAIN_RENDERER_LOST.load(Ordering::Acquire)
            };
            if confirmed {
                self.quarantine_renderers();
            } else {
                std::panic::resume_unwind(panic);
            }
        }
    }
    fn quarantine_renderers(&mut self) {
        for (index, lost, id) in [
            (0, MAIN_RENDERER_LOST.load(Ordering::Acquire), self.main_id),
            (
                1,
                PREVIEW_RENDERER_LOST.load(Ordering::Acquire),
                self.preview_id,
            ),
        ] {
            if lost && !self.quarantined[index] {
                self.quarantined[index] = true;
                crate::app_state().preview.pause(true);
                if index == 0 {
                    crate::app_state().session.cancel_active();
                    crate::app_state().preview.cancel();
                }
                if let Some(view) = id.and_then(|id| self.inner.windows.get_mut(&id)) {
                    view.suspend();
                    view.window.set_title(
                        "GPU renderer lost · F5 restarts (resets controls) · Close exits",
                    );
                }
            }
        }
    }

    fn restart_renderer(&mut self, detached: bool, event_loop: &ActiveEventLoop) {
        if detached {
            if let Some(id) = self.preview_id.take() {
                self.inner.windows.remove(&id);
            }
            PREVIEW_RENDERER_LOST.store(false, Ordering::Release);
            self.quarantined[1] = false;
            self.reconcile_preview(event_loop);
        } else {
            crate::app_state().session.cancel_active();
            crate::app_state().preview.cancel();
            let _ = crate::app_state().preview.set_history_preview(None);
            set_preview_fullscreen(false);
            self.inner.windows.clear();
            self.preview_id = None;
            MAIN_RENDERER_LOST.store(false, Ordering::Release);
            PREVIEW_RENDERER_LOST.store(false, Ordering::Release);
            self.quarantined = [false; 2];
            let document =
                DioxusDocument::new(VirtualDom::new(crate::app), DocumentConfig::default());
            let attributes = Window::default_attributes()
                .with_title("Diaxus · Video Converter")
                .with_inner_size(winit::dpi::LogicalSize::new(960.0, 760.0))
                .with_min_inner_size(winit::dpi::LogicalSize::new(900.0, 740.0));
            let config = WindowConfig::with_attributes(
                Box::new(document),
                DioxusNativeWindowRenderer::new(),
                attributes,
            );
            let id = self.insert_document(config, event_loop);
            self.main_id = Some(id);
            *APP_WINDOW.lock().unwrap() = Some(self.inner.windows[&id].window.clone());
            self.inner.windows.get_mut(&id).unwrap().resume();
        }
    }
    fn insert_document(
        &mut self,
        config: WindowConfig<DioxusNativeWindowRenderer>,
        event_loop: &ActiveEventLoop,
    ) -> WindowId {
        let mut view = View::init(config, event_loop, &self.inner.proxy);
        let renderer = view.renderer.clone();
        let doc = view.downcast_doc_mut::<DioxusDocument>();
        doc.vdom
            .in_scope(ScopeId::ROOT, || provide_context(renderer));
        doc.initial_build();
        view.request_redraw();
        let id = view.window_id();
        self.inner.windows.insert(id, view);
        id
    }
    fn reconcile_preview(&mut self, event_loop: &ActiveEventLoop) {
        match (preview_window_open(), self.preview_id) {
            (true, None) => {
                let document = DioxusDocument::new(
                    VirtualDom::new(crate::preview_window),
                    DocumentConfig::default(),
                );
                let attributes = WindowAttributes::default()
                    .with_title("Diaxus · Video Preview")
                    .with_inner_size(winit::dpi::LogicalSize::new(960.0, 760.0))
                    .with_min_inner_size(winit::dpi::LogicalSize::new(640.0, 480.0))
                    .with_maximized(true);
                let config = WindowConfig::with_attributes(
                    Box::new(document),
                    DioxusNativeWindowRenderer::new(),
                    attributes,
                );
                let id = self.insert_document(config, event_loop);
                self.preview_id = Some(id);
                self.inner.windows.get_mut(&id).unwrap().resume();
            }
            (false, Some(id)) => {
                self.inner.windows.remove(&id);
                self.preview_id = None;
                if let Some(id) = self.main_id
                    && let Some(view) = self.inner.windows.get(&id)
                {
                    view.window.focus_window();
                    view.request_redraw();
                }
            }
            _ => {}
        }
    }
}

impl ApplicationHandler<BlitzShellEvent> for ViewportApplication {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if !self.initialized
            && let Some(monitor) = event_loop.primary_monitor()
        {
            self.scale = monitor.scale_factor();
        }
        self.initialized = true;
        // The pinned Dioxus application hides its window handle. Use Blitz's
        // public bootstrap with the same document/renderer and renderer context
        // so expanded preview can use winit, without an OS-specific handle workaround.
        // This app uses inline styles and no document/history provider calls.
        if let Some(config) = self.pending.take() {
            let id = self.insert_document(config, event_loop);
            self.main_id = Some(id);
            *APP_WINDOW.lock().unwrap() = Some(self.inner.windows[&id].window.clone());
        }
        for (id, view) in &mut self.inner.windows {
            let index = usize::from(self.preview_id == Some(*id));
            if !self.quarantined[index] {
                view.resume();
            }
        }
    }
    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        self.inner.suspended(event_loop);
    }
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        self.inner.new_events(event_loop, cause);
    }
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: BlitzShellEvent) {
        self.quarantine_renderers();
        self.inner.user_event(event_loop, event);
        self.reconcile_preview(event_loop);
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        self.quarantine_renderers();
        let detached = self.preview_id == Some(id);
        // Late events from a dropped preview must never close the main window
        // or overwrite its viewport measurements.
        if !detached && self.main_id != Some(id) {
            return;
        }
        if matches!(event, WindowEvent::CloseRequested) {
            if detached {
                set_preview_fullscreen(false);
                self.reconcile_preview(event_loop);
                return;
            }
            // Main-window close ends the application, even with preview open.
            PREVIEW_FULLSCREEN.store(false, Ordering::Release);
            self.inner.windows.clear();
            *APP_WINDOW.lock().unwrap() = None;
            event_loop.exit();
            return;
        }
        if self.quarantined[usize::from(detached)] {
            if let WindowEvent::KeyboardInput { event, .. } = &event
                && event.state == ElementState::Pressed
                && !event.repeat
                && event.logical_key == Key::Named(NamedKey::F5)
            {
                self.restart_renderer(detached, event_loop);
            }
            return; // Never submit drawing/resizing to the lost device.
        }
        if detached {
            if let Some(view) = self.inner.windows.get(&id) {
                let size = view
                    .window
                    .inner_size()
                    .to_logical::<f64>(view.window.scale_factor());
                PREVIEW_WIDTH.store(size.width.to_bits(), Ordering::Release);
                PREVIEW_HEIGHT.store(size.height.to_bits(), Ordering::Release);
            }
            if let WindowEvent::KeyboardInput { event, .. } = &event
                && event.state == ElementState::Pressed
            {
                if event.logical_key == Key::Named(NamedKey::Escape) {
                    set_preview_fullscreen(false);
                    self.reconcile_preview(event_loop);
                    return;
                }
                if let Some(key) = player_key(&event.logical_key, self.modifiers)
                    && let Ok(mut keys) = PLAYER_KEYS.lock()
                {
                    keys.set_open(true);
                    keys.push(key, event.repeat);
                    return;
                }
            }
            if let WindowEvent::ModifiersChanged(modifiers) = &event {
                self.modifiers = modifiers.state();
            }
            if matches!(event, WindowEvent::Focused(false)) {
                self.modifiers = ModifiersState::empty();
            }
            self.dispatch_window_event(event_loop, id, event);
            self.reconcile_preview(event_loop);
            return;
        }
        match &event {
            WindowEvent::Resized(size) => {
                self.physical_width = size.width;
                self.physical_height = size.height;
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => self.scale = *scale_factor,
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::Focused(false) => {
                set_color_focus(None);
                self.modifiers = ModifiersState::empty();
                if let Ok(mut keys) = PLAYER_KEYS.lock() {
                    keys.pending.clear();
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if event.logical_key == Key::Named(NamedKey::Escape)
                    && PREVIEW_FULLSCREEN.load(Ordering::Acquire)
                {
                    if let Ok(mut keys) = PLAYER_KEYS.lock() {
                        keys.set_open(true);
                        keys.push(PlayerKey::Close, false);
                    }
                    return;
                }
                // Tab moves focus to ordinary controls. Re-enable shortcuts by
                // clicking the inline player, never intercept path-field typing.
                if event.logical_key == Key::Named(NamedKey::Tab) {
                    set_player_open(false);
                    set_color_focus(None);
                }
                if let Some(key) = color_key(&event.logical_key, self.modifiers)
                    && let Ok(mut keys) = COLOR_KEYS.lock()
                    && let Some(index) = keys.focused
                {
                    if keys.pending.len() < 16 {
                        keys.pending.push_back((index, key));
                    }
                    return;
                }
                if let Some(key) = player_key(&event.logical_key, self.modifiers)
                    && let Ok(mut keys) = PLAYER_KEYS.lock()
                    && keys.push(key, event.repeat)
                {
                    // Do not also activate a focused DOM button (e.g. Space).
                    return;
                }
            }
            _ => {}
        }
        LOGICAL_WIDTH.store(
            (f64::from(self.physical_width) / self.scale).to_bits(),
            Ordering::Release,
        );
        LOGICAL_HEIGHT.store(
            (f64::from(self.physical_height) / self.scale).to_bits(),
            Ordering::Release,
        );
        self.dispatch_window_event(event_loop, id, event);
        self.reconcile_preview(event_loop);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fullscreen_preview_fits_image_and_controls_without_distorting_aspect() {
        for (width, height) in [(960.0, 760.0), (1920.0, 1080.0), (1280.0, 720.0)] {
            let (image_width, image_height) = fit_preview(width, height);
            assert!(image_width <= width - 48.0);
            assert!(image_height <= height - 210.0);
            assert!((image_width / image_height - 16.0 / 9.0).abs() < 0.0001);
            let left = (width - image_width) / 2.0;
            assert!(fullscreen_fraction(left, width, height).abs() < 0.0001);
            assert!((fullscreen_fraction(width / 2.0, width, height) - 0.5).abs() < 0.0001);
            assert!((fullscreen_fraction(left + image_width, width, height) - 1.0).abs() < 0.0001);
        }
    }

    #[test]
    fn timeline_coordinates_match_centered_layout() {
        for (width, left, track_width) in [
            (900.0, 149.0, 368.0),
            (960.0, 149.0, 428.0),
            (1280.0, 239.0, 568.0),
        ] {
            assert_eq!(color_fraction_in_viewport(left - 20.0, width), 0.0);
            assert_eq!(color_fraction_in_viewport(left, width), 0.0);
            assert_eq!(
                color_fraction_in_viewport(left + track_width / 2.0, width),
                0.5
            );
            assert_eq!(color_fraction_in_viewport(left + track_width, width), 1.0);
            assert_eq!(
                color_fraction_in_viewport(left + track_width + 20.0, width),
                1.0
            );
        }
        assert_eq!(timeline_fraction(611.0), 0.0);
        assert_eq!(timeline_fraction(771.0), 0.5);
        assert_eq!(timeline_fraction(931.0), 1.0);
        assert_eq!(fraction_in_viewport(711.0, 900.0), 0.5);
        assert_eq!(fraction_in_viewport(1001.0, 1280.0), 0.5);
        assert_eq!(fraction_in_viewport(1661.0, 2600.0), 0.5);
    }

    #[test]
    fn native_player_keys_are_active_only_bounded_and_do_not_repeat_toggles() {
        let mut keys = PlayerKeys::default();
        assert!(!keys.push(PlayerKey::Toggle, false));
        keys.set_open(true);
        keys.push(PlayerKey::Toggle, false);
        keys.push(PlayerKey::Toggle, true);
        assert_eq!(keys.pending.pop_front(), Some(PlayerKey::Toggle));
        assert!(keys.pending.is_empty());
        for _ in 0..100 {
            keys.push(PlayerKey::Forward, true);
        }
        assert_eq!(keys.pending.len(), 16);
        // Escape must not get stuck behind a full repeat queue.
        keys.push(PlayerKey::Close, false);
        assert_eq!(keys.pending.pop_front(), Some(PlayerKey::Close));
        assert!(keys.pending.is_empty());
        keys.push(PlayerKey::Back, false);
        keys.set_open(false);
        keys.set_open(true);
        assert!(
            keys.pending.is_empty(),
            "old player commands must not survive reopening"
        );
        assert_eq!(
            player_key(&Key::Named(NamedKey::ArrowRight), ModifiersState::empty()),
            Some(PlayerKey::Forward)
        );
        assert_eq!(
            player_key(&Key::Named(NamedKey::Space), ModifiersState::CONTROL),
            None
        );
        assert_eq!(
            player_key(&Key::Character("a".into()), ModifiersState::empty()),
            None
        );
    }
}
