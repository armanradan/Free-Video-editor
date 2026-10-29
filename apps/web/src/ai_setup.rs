use dioxus::prelude::*;
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::{JsCast, closure::Closure};

#[component]
pub fn AiSetup(
    disabled: bool,
    selected: media_core::ai_upscale::AiSettings,
    choices: ui::AiChoices,
    on_select: EventHandler<media_core::ai_upscale::AiSettings>,
) -> Element {
    let mut busy = use_signal(|| false);
    let mut status =
        use_signal(|| "No assets checked. Setup runs only when you choose an action.".to_string());
    let cancelled = use_hook(|| Rc::new(Cell::new(false)));
    let mounted = use_hook(|| Rc::new(Cell::new(true)));
    let stop = cancelled.clone();
    let alive = mounted.clone();
    use_drop(move || {
        stop.set(true);
        alive.set(false);
    });
    let stop = cancelled.clone();
    let action = EventHandler::new(move |kind: u8| {
        if disabled || *busy.peek() {
            return;
        }
        busy.set(true);
        status.set("Working on local AI setup…".into());
        cancelled.set(false);
        let cancelled = cancelled.clone();
        let mounted = mounted.clone();
        spawn(async move {
            let check = Closure::<dyn FnMut() -> bool>::new(move || cancelled.get());
            let result: Result<String, media_core::MediaError> = match kind {
                0 => media_web::import_ai_model_from_input(
                    "ai-model-file",
                    check.as_ref().unchecked_ref(),
                )
                .await
                .map(|_| "Model hash verified and stored locally.".into()),
                1 => media_web::read_ai_model()
                    .await
                    .map(|_| "Stored model hash verified.".into()),
                2 => media_web::remove_ai_model().await.map(|found| {
                    if found {
                        "Stored model removed."
                    } else {
                        "No stored model."
                    }
                    .into()
                }),
                3 => media_web::acquire_ai_runtime(check.as_ref().unchecked_ref())
                    .await
                    .map(|_| "ONNX Runtime Web 1.30.0 verified and cached.".into()),
                _ => media_web::remove_ai_runtime().await.map(|found| {
                    if found {
                        "Stored runtime removed. Reload to discard any previously loaded code."
                    } else {
                        "No stored runtime."
                    }
                    .into()
                }),
            };
            if mounted.get() {
                status.set(match result {
                    Ok(message) => message,
                    Err(error) => format!("Setup failed: {error}"),
                });
                busy.set(false);
            }
        });
    });
    rsx! {
        ui::AiUpscalingPanel { native:false, selected, choices, on_select, disabled:disabled||busy(),
            div { class:"ai-setup",
                label { r#for:"ai-model-file", "Validated ONNX model" }
                input { id:"ai-model-file", r#type:"file", accept:".onnx", disabled:disabled||busy() }
                div { class:"ai-actions",
                    button { id:"ai-import-model", disabled:disabled||busy(), onclick:move |_|action.call(0), "Import model" }
                    button { id:"ai-check-model", disabled:disabled||busy(), onclick:move |_|action.call(1), "Check stored model" }
                    button { id:"ai-remove-model", disabled:disabled||busy(), onclick:move |_|action.call(2), "Remove model" }
                }
                p { class:"note", "Browser runtime: ONNX Runtime Web 1.30.0 · 26,901,320 bytes · MIT · jsDelivr. Download requires an explicit action; videos stay on this device." }
                div { class:"ai-actions",
                    button { id:"ai-acquire-runtime", disabled:disabled||busy(), onclick:move |_|action.call(3), "Download runtime" }
                    button { id:"ai-remove-runtime", disabled:disabled||busy(), onclick:move |_|action.call(4), "Remove runtime" }
                    if busy() { button { id:"ai-cancel-setup", onclick:move |_|stop.set(true), "Cancel setup" } }
                }
                p { id:"ai-setup-status", class:"note", role:"status", "{status}" }
            }
        }
    }
}
