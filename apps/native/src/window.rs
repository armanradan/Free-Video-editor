//! Thin native event-loop adapter for logical viewport measurements.
//! Dioxus Native 0.7.10's element_coordinates/get_client_rect are unimplemented.
use blitz_shell::{BlitzShellEvent, WindowConfig};
use dioxus::native::{
    DioxusDocument, DioxusNativeApplication, DioxusNativeWindowRenderer, DocumentConfig,
};
use dioxus::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use winit::{
    application::ApplicationHandler,
    event::{StartCause, WindowEvent},
    event_loop::ActiveEventLoop,
    window::{WindowAttributes, WindowId},
};

static LOGICAL_WIDTH: AtomicU64 = AtomicU64::new(960.0_f64.to_bits());

pub fn timeline_fraction(client_x: f64) -> f64 {
    let width = f64::from_bits(LOGICAL_WIDTH.load(Ordering::Acquire));
    fraction_in_viewport(client_x, width)
}

fn fraction_in_viewport(client_x: f64, width: f64) -> f64 {
    // The centered panel's border-box is 676px; its content is exactly 640px.
    let left = (width - 676.0) / 2.0 + 17.0;
    (client_x - left) / 640.0
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
    };
    event_loop.run_app(&mut application).unwrap();
}

struct ViewportApplication {
    inner: DioxusNativeApplication,
    physical_width: u32,
    scale: f64,
    initialized: bool,
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
        assert_eq!(timeline_fraction(159.0), 0.0);
        assert_eq!(timeline_fraction(479.0), 0.5);
        assert_eq!(timeline_fraction(799.0), 1.0);
        // Centered placement must move with a resized logical viewport.
        assert_eq!(fraction_in_viewport(639.0, 1280.0), 0.5);
        assert_eq!(fraction_in_viewport(1299.0, 2600.0), 0.5);
    }
}
