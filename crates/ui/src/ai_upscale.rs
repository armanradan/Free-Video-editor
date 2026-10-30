use dioxus::prelude::*;
use media_core::ai_upscale::{AiCapability, AiModel, AiScale, AiSettings};

/// Metadata supplied by the platform after acceptance, never inferred from a
/// successful cache import or codec probe. Each scale has its own exact gate.
#[derive(Clone, Debug, PartialEq)]
pub struct AiChoices {
    pub two: AiCapability,
    pub four: AiCapability,
}

impl Default for AiChoices {
    fn default() -> Self {
        let pending = AiCapability::PilotOnly {
            reason:
                "Preview, playback and quality acceptance are incomplete. Setup does not enable AI."
                    .into(),
        };
        Self {
            two: pending.clone(),
            four: pending,
        }
    }
}

pub fn onnx_selection(scale: AiScale) -> AiSettings {
    AiSettings::OnnxRuntime {
        model: AiModel::RealEsrGeneralX4v3StrongFp32V1,
        scale,
    }
}

pub fn rtx_selection(scale: AiScale) -> AiSettings {
    AiSettings::RtxVideo {
        configuration: media_core::ai_upscale::RtxConfiguration::Sdk11SdrV1,
        scale,
    }
}

impl AiChoices {
    /// Used by the view and again by the app command handler. Off always remains
    /// a recovery action; dimensions/format/route are checked by platform jobs.
    pub fn select(&self, requested: AiSettings) -> Result<AiSettings, String> {
        if requested == AiSettings::Off {
            return Ok(requested);
        }
        let capability = self.capability(requested)?;
        match capability {
            AiCapability::Validated { settings, .. } if *settings == requested => Ok(requested),
            AiCapability::Validated { .. } => {
                Err("AI acceptance does not match the requested backend/model/scale.".into())
            }
            AiCapability::PilotOnly { reason } | AiCapability::Unavailable { reason } => {
                Err(reason.clone())
            }
        }
    }

    pub fn capability(&self, requested: AiSettings) -> Result<&AiCapability, String> {
        match requested {
            AiSettings::OnnxRuntime {
                scale: AiScale::Two,
                ..
            }
            | AiSettings::RtxVideo {
                scale: AiScale::Two,
                ..
            } => Ok(&self.two),
            AiSettings::OnnxRuntime {
                scale: AiScale::Four,
                ..
            }
            | AiSettings::RtxVideo {
                scale: AiScale::Four,
                ..
            } => Ok(&self.four),
            _ => Err("This AI backend/model is unavailable in these controls.".into()),
        }
    }
}

/// Setup is useful before release acceptance, but is never a capability grant.
#[component]
pub fn AiUpscalingPanel(
    native: bool,
    #[props(default)] rtx: bool,
    #[props(default)] disabled: bool,
    #[props(default)] on_expand: EventHandler<bool>,
    #[props(default)] selected: AiSettings,
    #[props(default)] choices: AiChoices,
    #[props(default)] on_select: EventHandler<AiSettings>,
    children: Element,
) -> Element {
    let mut expanded = use_signal(|| false);
    let rtx = native && rtx;
    let two = if rtx {
        rtx_selection(AiScale::Two)
    } else {
        onnx_selection(AiScale::Two)
    };
    let four = if rtx {
        rtx_selection(AiScale::Four)
    } else {
        onnx_selection(AiScale::Four)
    };
    let backend_label = if rtx {
        "RTX Video SDK / High3"
    } else {
        "ONNX Runtime / Real-ESRGAN"
    };
    let two_ready = choices.select(two).is_ok();
    let four_ready = choices.select(four).is_ok();
    let two_choices = choices.clone();
    let four_choices = choices.clone();
    let label = selected.scale().map_or_else(
        || "Off".to_string(),
        |scale| {
            format!(
                "{}× · {}",
                scale.factor(),
                if rtx { "RTX Video" } else { "ONNX Runtime" }
            )
        },
    );
    let availability = if selected != AiSettings::Off && choices.select(selected).is_err() {
        format!(
            "Selected AI is unavailable: {} Off restores ordinary resize.",
            choices.select(selected).unwrap_err()
        )
    } else if selected == AiSettings::Off {
        match (choices.select(two), choices.select(four)) {
            (Ok(_), Ok(_)) => {
                "Validated 2× and 4× are available. Set up the required assets, then choose a scale explicitly.".into()
            }
            (Ok(_), Err(reason)) => format!("2× is available. 4× unavailable: {reason}"),
            (Err(reason), Ok(_)) => format!("4× is available. 2× unavailable: {reason}"),
            (Err(two), Err(four)) if two == four => two,
            (Err(two), Err(four)) => format!("2× unavailable: {two} 4× unavailable: {four}"),
        }
    } else if rtx {
        "AI replaces output sizing. Ordinary resize settings are retained; Off restores them. RTX runs at the selected native scale.".into()
    } else {
        "AI replaces output sizing. Ordinary resize settings are retained; Off restores them. 2× uses the model's 4× result followed by shared downscaling.".into()
    };
    let body = rsx! {
                    p { class: "note", if selected == AiSettings::Off {"AI is Off. Ordinary resizing and color controls remain available."} else {"AI is selected. Reset color and Before do not change its conversion settings."} }
                    div { class: "ai-scales", role: "group", aria_label: "AI scale",
                        button { id: "ai-off", r#type: "button", disabled:disabled.then_some("true"), aria_pressed:selected == AiSettings::Off,
                            onclick:move |_|{if !disabled {on_select.call(AiSettings::Off)}}, "Off" }
                        button { id: "ai-2x", r#type: "button", disabled:(disabled||!two_ready).then_some("true"), aria_pressed:selected == two,
                            onclick:move |_|{if !disabled && two_choices.select(two).is_ok(){on_select.call(two)}}, if two_ready {"2×"}else{"2× · unavailable"} }
                        button { id: "ai-4x", r#type: "button", disabled:(disabled||!four_ready).then_some("true"), aria_pressed:selected == four,
                            onclick:move |_|{if !disabled && four_choices.select(four).is_ok(){on_select.call(four)}}, if four_ready {"4×"}else{"4× · unavailable"} }
                    }
                    p { id: "ai-availability", class: "note", role: "status",
                        "{backend_label}: {availability}"
                    }
                    p { class: "note", "Burn is unavailable on the tested devices." }
                    if native && !rtx { p { class: "note", "RTX Video: import an explicit local RTX configuration to inspect its status. Ordinary UI and release acceptance remain pending." } }
                    if rtx {
                        p { class:"note", "NVIDIA RTX Video SDK 1.1 · SDR High3 · 2× input up to 1280×720; 4× up to 640×360. Requires separately installed SDK, evaluated NVIDIA GPU and acceptance of the supplied SDK terms. No Real-ESRGAN model is used." }
                        p { class:"note", "Preview is sampled and may take several seconds to prepare. Real-time playback and audio synchronization have not been validated." }
                    } else {
                    p { class: "note",
                        "Model: realesr-general-x4v3 · strong denoise · 4,866,413 bytes · BSD-3-Clause. Import only the validated ONNX export."
                    }
                    p { class:"note",
                        a { href:"https://github.com/xinntao/Real-ESRGAN/blob/a4abfb2979a7bbff3f69f58f58ae324608821e27/docs/model_zoo.md", "Official model source" }
                        " · "
                        a { href:"https://github.com/xinntao/Real-ESRGAN/blob/a4abfb2979a7bbff3f69f58f58ae324608821e27/LICENSE", "Model license" }
                    }
                    if two_ready || four_ready { p { class:"note", "Validated source limit: 577×579, opaque 8-bit SDR. This image model can smooth detail, alter text and flicker. Preview speed depends on the GPU." } }
                    }
                    {children}
    };
    rsx! {
        section { class:"panel ai-panel", aria_label:"AI Upscaling",
            button { id:"ai-expand", class:"color-toggle", r#type:"button",
                disabled:disabled.then_some("true"), aria_expanded:expanded(),aria_controls:"ai-settings",
                onclick:move |_|{let next=!expanded();expanded.set(next);on_expand.call(next);},
                span { if expanded(){"− AI Upscaling"}else{"+ AI Upscaling"} }
                span { class:"note", "{label}" }
            }
            if expanded(){
                if native {
                    section { id:"ai-settings",class:"ai-setup-screen",aria_label:"AI Upscaling setup",
                        div { class:"picker-heading",
                            h2 {"AI Upscaling"}
                            button { id:"ai-close",disabled:disabled.then_some("true"),onclick:move |_|{expanded.set(false);on_expand.call(false);},"Done" }
                        }
                        {body}
                    }
                }else{
                    div {id:"ai-settings",{body}}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus::core::{ElementId, Mutation, Mutations, ScopeId};
    use media_core::{Size, VideoSettings, ai_upscale::AiGeometryLimits};
    use std::{
        any::Any,
        cell::{Cell, RefCell},
        rc::Rc,
    };

    struct State {
        native: bool,
        rtx: bool,
        selected: Cell<AiSettings>,
        choices: RefCell<AiChoices>,
        busy: Cell<bool>,
        events: Cell<usize>,
        scope: Cell<Option<ScopeId>>,
    }

    fn root() -> Element {
        let state = use_context::<Rc<State>>();
        state.scope.set(Some(dioxus::core::current_scope_id()));
        let selected = state.selected.get();
        let choices = state.choices.borrow().clone();
        let native = state.native;
        let rtx = state.rtx;
        let disabled = state.busy.get();
        rsx! { AiUpscalingPanel { native, rtx, selected, choices, disabled,
            on_select: move |value| {state.selected.set(value);state.events.set(state.events.get()+1);},
        } }
    }

    fn clicks(mutations: Mutations) -> Vec<ElementId> {
        mutations
            .edits
            .into_iter()
            .filter_map(|edit| match edit {
                Mutation::NewEventListener { name, id } if name == "click" => Some(id),
                _ => None,
            })
            .collect()
    }

    fn click(dom: &VirtualDom, id: ElementId) {
        let event = Event::new(
            Rc::new(PlatformEventData::new(Box::<
                dioxus_html::SerializedMouseData,
            >::default())) as Rc<dyn Any>,
            true,
        );
        dom.runtime().handle_event("click", event, id);
    }

    #[test]
    fn actual_panel_events_enforce_exact_acceptance_and_keep_snapshots() {
        dioxus_html::set_event_converter(Box::new(dioxus_html::SerializedHtmlEventConverter));
        for (native, rtx) in [(false, false), (true, false), (true, true), (false, true)] {
            let state = Rc::new(State {
                native,
                rtx,
                selected: Cell::new(AiSettings::Off),
                choices: RefCell::new(AiChoices::default()),
                busy: Cell::new(false),
                events: Cell::new(0),
                scope: Cell::new(None),
            });
            let mut dom = VirtualDom::new(root);
            dom.insert_any_root_context(Box::new(state.clone()));
            let expand = clicks(dom.rebuild_to_vec())[0];
            click(&dom, expand);
            let events = clicks(dom.render_immediate_to_vec());
            let buttons = &events[events.len() - 3..]; // Off/2x/4x; native Done precedes the shared body.
            click(&dom, buttons[1]);
            click(&dom, buttons[2]);
            assert_eq!(state.selected.get(), AiSettings::Off);
            assert_eq!(
                state.events.get(),
                0,
                "disabled-event dispatch bypassed a pilot gate"
            );
            let two = if native && rtx {
                rtx_selection(AiScale::Two)
            } else {
                onnx_selection(AiScale::Two)
            };
            let four = if native && rtx {
                rtx_selection(AiScale::Four)
            } else {
                onnx_selection(AiScale::Four)
            };
            let limits = AiGeometryLimits {
                max_source: Size::new(64, 36).unwrap(),
                max_output: Size::new(256, 144).unwrap(),
                max_output_pixels: 256 * 144,
            };
            // Test-only acceptance metadata exercises view events; no platform
            // runtime, release capability or application setting is granted.
            state.choices.borrow_mut().two = AiCapability::Validated {
                settings: two,
                limits,
            };
            state.choices.borrow_mut().four = AiCapability::Validated {
                settings: two,
                limits,
            }; // mismatched scale
            dom.mark_dirty(state.scope.get().unwrap());
            dom.render_immediate_to_vec();
            click(&dom, buttons[2]);
            assert_eq!(state.selected.get(), AiSettings::Off);
            click(&dom, buttons[1]);
            assert_eq!(state.selected.get(), two);
            let snapshot = VideoSettings {
                ai: state.selected.get(),
                ..Default::default()
            };
            state.busy.set(true);
            dom.mark_dirty(state.scope.get().unwrap());
            dom.render_immediate_to_vec();
            click(&dom, buttons[0]);
            assert_eq!(
                state.selected.get(),
                two,
                "busy control accepted a forged click"
            );
            state.busy.set(false);
            state.choices.borrow_mut().four = AiCapability::Validated {
                settings: four,
                limits,
            };
            dom.mark_dirty(state.scope.get().unwrap());
            dom.render_immediate_to_vec();
            click(&dom, buttons[2]);
            assert_eq!(state.selected.get(), four);
            click(&dom, buttons[0]);
            assert_eq!(state.selected.get(), AiSettings::Off);
            assert_eq!(
                snapshot.ai, two,
                "later selection mutated an existing job snapshot"
            );
        }
    }
}
