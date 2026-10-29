use dioxus::prelude::*;
use media_native::{CancellationToken, ai_assets::ModelCache, ai_runtime::RuntimePaths};

pub fn pending_choices() -> ui::AiChoices {
    let pending = media_core::ai_upscale::AiCapability::PilotOnly {
        reason: "Ordinary native window acceptance is pending. Service, export, handoff and quality/performance measurements are recorded for evaluated configurations; setup alone cannot enable 2×/4×.".into(),
    };
    ui::AiChoices {
        two: pending.clone(),
        four: pending,
    }
}

#[component]
pub fn AiSetup(
    disabled: bool,
    mut expanded: Signal<bool>,
    selected: media_core::ai_upscale::AiSettings,
    choices: ui::AiChoices,
    #[props(default)] rtx: bool,
    on_select: EventHandler<media_core::ai_upscale::AiSettings>,
    on_configuration: EventHandler<Option<media_native::ai_configuration::NativeAiConfiguration>>,
    on_busy: EventHandler<bool>,
) -> Element {
    let mut cache = use_signal(String::new);
    let mut runtime = use_signal(String::new);
    let mut status =
        use_signal(|| "Choose a local cache folder to prepare the validated model.".to_string());
    let mut busy = use_signal(|| false);
    let mut cancellation = use_signal(|| None::<CancellationToken>);
    let import_configuration = move |_| {
        if disabled || *busy.peek() {
            return;
        }
        busy.set(true);
        on_busy.call(true);
        status.set("Choose an existing local AI configuration. Import does not execute a runtime or accept SDK terms.".into());
        spawn(async move {
            let (send, receive) = futures_channel::oneshot::channel();
            std::thread::spawn(move || {
                let result = rfd::FileDialog::new()
                    .set_title("Import local AI configuration")
                    .add_filter("AI configuration", &["json"])
                    .pick_file()
                    .ok_or_else(|| "Configuration selection cancelled".to_string())
                    .and_then(|path| {
                        media_native::ai_configuration::NativeAiConfiguration::read(&path)
                            .map_err(|e| e.to_string())
                    });
                let _ = send.send(result);
            });
            match receive.await {
                Ok(Ok(config)) => {
                    status.set(format!("Local configuration imported for {}. Select that processing GPU explicitly. Runtime execution, SDK terms and release acceptance are separate gates.", config.processing_adapter));
                    on_configuration.call(Some(config));
                }
                Ok(Err(error)) => status.set(error),
                Err(_) => status.set("Configuration worker ended without a result.".into()),
            }
            busy.set(false);
            on_busy.call(false);
        });
    };
    use_drop(move || {
        if let Some(token) = cancellation.peek().as_ref() {
            token.cancel();
        }
    });
    let action = EventHandler::new(move |kind: u8| {
        if disabled || *busy.peek() {
            return;
        }
        if (1..=3).contains(&kind) && cache.peek().is_empty() {
            status.set("Choose a model cache folder first.".into());
            return;
        }
        let directory = cache.peek().clone();
        let token = CancellationToken::default();
        cancellation.set(Some(token.clone()));
        busy.set(true);
        on_busy.call(true);
        if kind == 1 || kind == 3 {
            on_configuration.call(None);
        }
        status.set("Working on local AI setup…".into());
        spawn(async move {
            let (sender, receiver) = futures_channel::oneshot::channel();
            std::thread::spawn(move || {
                let result = (|| -> Result<(String, Option<String>, Option<String>), String> {
                    match kind {
                        0 => {
                            let path = rfd::FileDialog::new()
                                .set_title("Choose AI model cache folder")
                                .pick_folder()
                                .ok_or("Folder selection cancelled")?;
                            Ok((
                                "Model cache folder selected; no files downloaded or installed."
                                    .into(),
                                Some(path.to_string_lossy().into_owned()),
                                None,
                            ))
                        }
                        1 => {
                            let path = rfd::FileDialog::new()
                                .set_title("Import the validated ONNX model")
                                .add_filter("ONNX model", &["onnx"])
                                .pick_file()
                                .ok_or("Model selection cancelled")?;
                            ModelCache::new(&directory)
                                .import_onnx(&path, &token)
                                .map_err(|e| e.to_string())?;
                            Ok(("Model hash verified and stored locally.".into(), None, None))
                        }
                        2 => {
                            ModelCache::new(&directory)
                                .read_onnx(&token)
                                .map_err(|e| e.to_string())?;
                            Ok(("Stored model hash verified.".into(), None, None))
                        }
                        3 => {
                            let removed = ModelCache::new(&directory)
                                .remove_onnx()
                                .map_err(|e| e.to_string())?;
                            Ok((
                                if removed {
                                    "Stored model removed."
                                } else {
                                    "No stored model."
                                }
                                .into(),
                                None,
                                None,
                            ))
                        }
                        _ => {
                            let path = rfd::FileDialog::new()
                                .set_title("Choose an existing ONNX Runtime folder")
                                .pick_folder()
                                .ok_or("Runtime selection cancelled")?;
                            let report = RuntimePaths {
                                onnx_directory: Some(path.clone()),
                                rtx_sdk: None,
                            }
                            .inspect();
                            let message = report
                                .onnx
                                .iter()
                                .map(|component| {
                                    format!("{}: {}", component.status, component.reason)
                                })
                                .collect::<Vec<_>>()
                                .join(". ");
                            Ok((message, None, Some(path.to_string_lossy().into_owned())))
                        }
                    }
                })();
                let _ = sender.send(result);
            });
            match receiver.await {
                Ok(Ok((message, folder, ort))) => {
                    // Asset changes retire selected AI and its caches. A model
                    // hash check alone does not grant or revoke configuration.
                    if kind != 2 {
                        on_configuration.call(None);
                    }
                    if let Some(folder) = folder {
                        cache.set(folder);
                    }
                    if let Some(ort) = ort {
                        runtime.set(ort);
                    }
                    status.set(message);
                }
                Ok(Err(error)) => status.set(format!("Setup failed: {error}")),
                Err(_) => status.set("Setup worker ended without a result.".into()),
            }
            busy.set(false);
            on_busy.call(false);
            cancellation.set(None);
        });
    });
    rsx! {
        ui::AiUpscalingPanel { native:true, rtx, selected, choices, on_select, disabled:disabled||busy(), on_expand:move |opening|{expanded.set(opening);if opening{crate::app_state().preview.pause(true);crate::window::set_player_open(false);}},
            div { class:"ai-setup",
                p { class:"note", "Native runtimes are installed separately. These actions do not install a runtime, SDK, driver or toolchain." }
                div { class:"ai-actions",
                    button { id:"ai-import-configuration", disabled:(disabled||busy()).then_some("true"), onclick:import_configuration, "Import local AI configuration" }
                    button { id:"ai-clear-configuration", disabled:(disabled||busy()).then_some("true"), onclick:move |_|{if !disabled && !busy(){on_configuration.call(None);status.set("Local AI configuration cleared. AI is Off.".into());}}, "Clear configuration" }
                }
                p { class:"note", "Configuration selects existing assets and an exact GPU. It does not enable an unaccepted backend. RTX uses NVIDIA models; ONNX model actions below apply only to Real-ESRGAN." }
                p { id:"ai-cache-path", class:"note", if cache().is_empty(){"Model cache: not selected"}else{"Model cache: {cache}"} }
                div { class:"ai-actions",
                    button { id:"ai-choose-cache", disabled:(disabled||busy()).then_some("true"), onclick:move |_|action.call(0), "Choose cache folder" }
                    button { id:"ai-import-model", disabled:(disabled||busy()||cache().is_empty()).then_some("true"), onclick:move |_|action.call(1), "Import model" }
                    button { id:"ai-check-model", disabled:(disabled||busy()||cache().is_empty()).then_some("true"), onclick:move |_|action.call(2), "Check model" }
                    button { id:"ai-remove-model", disabled:(disabled||busy()||cache().is_empty()).then_some("true"), onclick:move |_|action.call(3), "Remove model" }
                }
                p { class:"note", if runtime().is_empty(){"ONNX Runtime: no folder selected"}else{"ONNX Runtime: {runtime}"} }
                div { class:"ai-actions",
                    button { id:"ai-choose-runtime", disabled:(disabled||busy()).then_some("true"), onclick:move |_|action.call(4), "Inspect runtime folder" }
                    if busy() && cancellation().is_some(){button { id:"ai-cancel-setup", onclick:move |_|{if let Some(token)=cancellation.peek().as_ref(){token.cancel();}}, "Cancel setup" }}
                }
                p { id:"ai-setup-status", class:"note", role:"status", "{status}" }
            }
        }
    }
}
