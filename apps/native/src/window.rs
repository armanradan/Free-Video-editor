//! Thin native event-loop adapter for logical viewport measurements.
//! Dioxus Native 0.7.10's element_coordinates/get_client_rect are unimplemented.
use blitz_shell::{BlitzShellEvent, WindowConfig};
use dioxus::native::{
    DioxusDocument, DioxusNativeApplication, DioxusNativeWindowRenderer, DocumentConfig,
};
use dioxus::prelude::*;
use std::{
    collections::VecDeque,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, StartCause, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{Key, ModifiersState, NamedKey},
    window::{WindowAttributes, WindowId},
};

static LOGICAL_WIDTH: AtomicU64 = AtomicU64::new(960.0_f64.to_bits());

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
    let inner = DioxusNativeApplication::new(event_loop.create_proxy(), config);
    let mut application = ViewportApplication {
        inner,
        physical_width: 960,
        scale: 1.0,
        initialized: false,
        modifiers: ModifiersState::empty(),
    };
    event_loop.run_app(&mut application).unwrap();
}

struct ViewportApplication {
    inner: DioxusNativeApplication,
    physical_width: u32,
    scale: f64,
    initialized: bool,
    modifiers: ModifiersState,
}

impl ApplicationHandler<BlitzShellEvent> for ViewportApplication {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if !self.initialized
            && let Some(monitor) = event_loop.primary_monitor()
        {
            self.scale = monitor.scale_factor();
        }
        self.initialized = true;
        self.inner.resumed(event_loop);
    }
    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        self.inner.suspended(event_loop);
    }
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        self.inner.new_events(event_loop, cause);
    }
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: BlitzShellEvent) {
        self.inner.user_event(event_loop, event);
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        match &event {
            WindowEvent::Resized(size) => self.physical_width = size.width,
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
        self.inner.window_event(event_loop, id, event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
