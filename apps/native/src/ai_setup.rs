use crate::ai_preferences::{AiPreferences, SavedSetup};
use dioxus::prelude::*;
use media_native::{CancellationToken, ai_assets::ModelCache, ai_runtime::RuntimePaths};

#[derive(Clone, Copy, PartialEq)]
enum SetupAction {
    Inspect,
    Test,
    Resume,
}

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
    configuration: Signal<Option<media_native::ai_configuration::NativeAiConfiguration>>,
    processing_adapter: Signal<String>,
    preview_path: Signal<String>,
    showing_preview: Signal<bool>,
    on_test: EventHandler<Option<media_native::ai_preflight::SetupPreflight>>,
    #[props(default)] rtx: bool,
    on_select: EventHandler<media_core::ai_upscale::AiSettings>,
    on_configuration: EventHandler<Option<media_native::ai_configuration::NativeAiConfiguration>>,
    on_busy: EventHandler<bool>,
) -> Element {
    let mut cache = use_signal(String::new);
    let mut runtime = use_signal(String::new);
    let mut advanced = use_signal(|| false);
    let mut status = use_signal(|| match AiPreferences::new().load_initial() {
        Ok(Some(_)) => "App AI configuration loaded. Open AI setup to check readiness.".into(),
        Ok(None) => "Import your local configuration once to set up AI.".into(),
        Err(e) => format!("Saved AI setup could not be read: {e}. Import configuration again."),
    });
    let mut busy = use_signal(|| false);
    let mut cancellation = use_signal(|| None::<CancellationToken>);
    let mut preflight_cancellation = use_signal(|| None::<CancellationToken>);
    let mut inspection = use_signal(|| None::<media_native::ai_preflight::SetupInspection>);
    let mut acknowledged_terms = use_signal(|| None::<String>);
    let mut auto_attempt = use_signal(|| {
        None::<(
            media_native::ai_configuration::NativeAiConfiguration,
            String,
        )>
    });
    use_effect(move || {
        let config = configuration();
        let _ = processing_adapter();
        inspection.set(None);
        acknowledged_terms.set(
            AiPreferences::new()
                .load()
                .ok()
                .flatten()
                .filter(|s| Some(&s.configuration) == config.as_ref())
                .and_then(|s| s.terms_sha256),
        );
        auto_attempt.set(None);
        on_test.call(None);
        if let Some(token) = preflight_cancellation.peek().as_ref() {
            token.cancel();
        }
    });
    let check_setup = EventHandler::new(move |action: SetupAction| {
        let execute = action != SetupAction::Inspect;
        if disabled || *busy.peek() {
            return;
        }
        let Some(config) = configuration() else {
            status.set("Import a local configuration first.".into());
            return;
        };
        if action == SetupAction::Test
            && inspection().is_none_or(|i| {
                !i.issues.is_empty()
                    || i.terms
                        .is_some_and(|t| acknowledged_terms().as_deref() != Some(t.sha256.as_str()))
            })
        {
            status.set("Inspect the current configuration and acknowledge its SDK terms before Test runtime.".into());
            return;
        }
        let adapter = processing_adapter();
        let terms = acknowledged_terms();
        on_test.call(None);
        let restore = (execute && showing_preview())
            .then(|| (preview_path(), crate::app_state().preview.state()));
        let handoff = if execute {
            match crate::app_state().preview.begin_conversion() {
                Ok(handoff) => Some(handoff),
                Err(error) => {
                    status.set(error);
                    return;
                }
            }
        } else {
            None
        };
        let token = CancellationToken::default();
        cancellation.set(Some(token.clone()));
        preflight_cancellation.set(Some(token.clone()));
        busy.set(true);
        on_busy.call(true);
        status.set(
            if execute {
                "Testing the configured runtime on tiny inputs at both scales…"
            } else {
                "Inspecting local runtime files without execution…"
            }
            .into(),
        );
        spawn(async move {
            let (send, receive) = futures_channel::oneshot::channel();
            let checked_config = config.clone();
            let checked_adapter = adapter.clone();
            std::thread::spawn(move || {
                let result = (|| -> Result<_, String> {
                    if let Some(handoff) = &handoff {
                        handoff.wait_retired(&token)?;
                    }
                    if token.is_cancelled() {
                        return Err("Setup cancelled".into());
                    }
                    let inspected = config.inspect_setup();
                    let test = || -> Result<_, String> {
                        let mut terms = terms;
                        if action == SetupAction::Resume {
                            let saved = AiPreferences::new()
                                .load()
                                .map_err(|e| e.to_string())?
                                .ok_or("Saved setup was cleared; import configuration again")?;
                            let hash = config.helper_sha256().map_err(|e| e.to_string())?;
                            if !saved.can_resume(&config, &inspected, &hash) {
                                return Err("Saved AI setup changed. Inspect configuration, review the SDK terms if changed, then Test runtime again.".into());
                            }
                            terms = saved.terms_sha256;
                        }
                        config
                            .test_setup(&adapter, terms.as_deref(), &token)
                            .map_err(|e| e.to_string())
                    };
                    let tested = if execute { Some(test()) } else { None };
                    if token.is_cancelled() {
                        return Err("Setup cancelled".into());
                    }
                    Ok((inspected, tested))
                })();
                drop(handoff);
                let _ = send.send(result);
            });
            // Configuration/GPU replacement cancels work and cannot publish
            // an old setup result or carry terms to a different configuration.
            let outcome = receive.await;
            if configuration().as_ref() == Some(&checked_config)
                && processing_adapter() == checked_adapter
            {
                match outcome {
                    Ok(Ok((inspected, tested))) => {
                        status.set(if let Some(Ok(report)) = tested {
                            let saved = SavedSetup { configuration:checked_config.clone(), terms_sha256:report.sdk_terms.as_ref().map(|t|t.sha256.clone()), approved_helper_sha256:Some(report.helper_sha256.clone()) };
                            let persistence = match AiPreferences::new().save(Some(&saved)) {Ok(())=>" Setup remembered for later launches.".into(),Err(e)=>format!(" Setup could not be saved: {e}.")};
                            let message=format!("RTX/AI runtime ready: {} input checks passed at 2×/4×.{} Inspect a supported source, then select a scale.", report.frames.len(), persistence);
                            on_test.call(Some(report));
                            message
                        } else if let Some(Err(error)) = tested {format!("Setup test failed: {error}")
                        } else if inspected.issues.is_empty() {
                            "Local component inspection passed. No runtime executed. Review the configured helper and SDK terms before Test runtime.".into()
                        } else {format!("Setup unavailable: {}", inspected.issues.join(". "))});
                        inspection.set(Some(inspected));
                    }
                    Ok(Err(error)) => status.set(format!("Setup test failed: {error}")),
                    Err(_) => status.set("Setup worker ended without a result.".into()),
                }
            } else {
                status.set(
                    "Setup result retired after configuration or GPU changed. Inspect again."
                        .into(),
                );
            }
            if let Some((path, previous)) = restore
                && showing_preview()
                && preview_path() == path
                && let Err(error) = crate::app_state().preview.start(
                    path.into(),
                    previous.position_us,
                    false,
                    previous.muted,
                )
            {
                status.set(format!("{} Preview restore failed: {error}", status()));
            }
            busy.set(false);
            on_busy.call(false);
            cancellation.set(None);
            preflight_cancellation.set(None);
        });
    });
    // Off startup stays lazy. Reopen setup on a later launch to validate only
    // a previously approved exact configuration/helper/terms, without a click
    // on Inspect or Test. Never deserialize a successful runtime result.
    use_effect(move || {
        let open = expanded();
        let config = configuration();
        let adapter = processing_adapter();
        if !open || disabled || *busy.peek() {
            return;
        }
        let Some(config) = config else {
            return;
        };
        if config.processing_adapter != adapter {
            return;
        }
        let key = (config.clone(), adapter);
        if auto_attempt.peek().as_ref() == Some(&key) {
            return;
        }
        auto_attempt.set(Some(key));
        if AiPreferences::new()
            .load()
            .ok()
            .flatten()
            .is_some_and(|s| s.configuration == config && s.approved_helper_sha256.is_some())
        {
            check_setup.call(SetupAction::Resume);
        } else {
            check_setup.call(SetupAction::Inspect);
        }
    });
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
                    let saved = SavedSetup {
                        configuration: config.clone(),
                        terms_sha256: None,
                        approved_helper_sha256: None,
                    };
                    status.set(match AiPreferences::new().save(Some(&saved)) {Ok(())=>format!("Configuration saved for {}. Inspect it, acknowledge SDK terms, then Test runtime & remember setup once.", config.processing_adapter), Err(e)=>format!("Configuration imported for this session. Could not save: {e}")});
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
        let mutation = kind == 1 || kind == 3;
        let restore = (mutation && showing_preview())
            .then(|| (preview_path(), crate::app_state().preview.state()));
        let handoff = if mutation {
            match crate::app_state().preview.begin_conversion() {
                Ok(handoff) => Some(handoff),
                Err(error) => {
                    status.set(error);
                    return;
                }
            }
        } else {
            None
        };
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
                    if let Some(handoff) = &handoff {
                        handoff.wait_retired(&token)?;
                    }
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
                drop(handoff);
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
            if let Some((path, previous)) = restore
                && showing_preview()
                && preview_path() == path
                && let Err(error) = crate::app_state().preview.start(
                    path.into(),
                    previous.position_us,
                    false,
                    previous.muted,
                )
            {
                status.set(format!("{} Preview restore failed: {error}", status()));
            }
            busy.set(false);
            on_busy.call(false);
            cancellation.set(None);
        });
    });
    rsx! {
        ui::AiUpscalingPanel { native:true, rtx, selected, choices, on_select, disabled:disabled||busy(), on_expand:move |opening|{expanded.set(opening);if opening{crate::app_state().preview.pause(true);crate::window::set_player_open(false);}},
            div { class:"ai-setup",
                p { class:"note", if configuration().is_some(){"RTX/AI component locations are managed by the app. Setup and SDK acknowledgement are remembered."}else{"RTX setup is missing from this installation. Deploy the app's RTX component asset; advanced configuration is available below."} }
                button {onclick:move |_|advanced.set(!advanced()),if advanced(){"Hide advanced setup"}else{"Advanced setup…"}}
                if advanced() {
                div { class:"ai-actions",
                    button { id:"ai-import-configuration", disabled:(disabled||busy()).then_some("true"), onclick:import_configuration, "Import local AI configuration" }
                    button { id:"ai-clear-configuration", disabled:(disabled||busy()).then_some("true"), onclick:move |_|{if !disabled && !busy(){let result=AiPreferences::new().save(None);on_configuration.call(None);status.set(match result {Ok(())=>"Saved AI setup cleared. AI is Off.".into(),Err(e)=>format!("AI is Off. Could not clear saved setup: {e}")});}}, "Clear saved setup" }
                }
                p { class:"note", "Configuration selects existing assets and an exact GPU. It does not enable an unaccepted backend. RTX uses NVIDIA models; ONNX model actions below apply only to Real-ESRGAN." }
                }
                div { class:"ai-actions",
                    if advanced() {
                    button { id:"ai-inspect-configuration", disabled:(disabled||busy()||configuration().is_none()).then_some("true"), onclick:move |_|check_setup.call(SetupAction::Inspect), "Inspect configuration" }
                    }
                    button { id:"ai-test-runtime", disabled:(disabled||busy()||configuration().is_none()||inspection().is_none_or(|i|!i.issues.is_empty()||(i.terms.is_some()&&acknowledged_terms().is_none()))).then_some("true"), onclick:move |_|check_setup.call(SetupAction::Test), "Test runtime & remember setup" }
                }
                if let Some(terms) = inspection().and_then(|i|i.terms) {
                    p { class:"note", "Read the supplied NVIDIA SDK terms: {terms.path.display()} · SHA-256 {terms.sha256}" }
                    div { class:"ai-actions",
                        button { id:"ai-sdk-consent", r#type:"button", aria_pressed:acknowledged_terms().as_deref()==Some(terms.sha256.as_str()), disabled:(disabled||busy()).then_some("true"),
                            onclick:move |_|{if !disabled && !busy(){let accepted=acknowledged_terms().is_some();let hash=if accepted{None}else{Some(terms.sha256.clone())};acknowledged_terms.set(hash.clone());on_test.call(None);if let Some(config)=configuration(){let saved=SavedSetup{configuration:config,terms_sha256:hash,approved_helper_sha256:None};status.set(match AiPreferences::new().save(Some(&saved)){Ok(())=>if accepted{"SDK acknowledgement revoked and saved approval removed. AI is Off.".into()}else{"SDK acknowledgement remembered. Run Test runtime & remember setup once.".into()},Err(e)=>format!("SDK choice changed for this session; could not save: {e}")});}}} ,
                            if acknowledged_terms().is_some() {"SDK terms accepted · revoke"} else {"Accept supplied SDK terms and remember"}
                        }
                    }
                }
                p { class:"note", "Test runtime remembers this helper and SDK acknowledgement. On later launches, opening AI setup automatically rechecks that exact setup using synthetic images. Changed files require review. AI stays Off until you select a scale." }
                if advanced() {
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
                }
                }
                if busy() && cancellation().is_some(){button { id:"ai-cancel-setup", onclick:move |_|{if let Some(token)=cancellation.peek().as_ref(){token.cancel();}}, "Cancel setup" }}
                p { id:"ai-setup-status", class:"note", role:"status", "{status}" }
            }
        }
    }
}
