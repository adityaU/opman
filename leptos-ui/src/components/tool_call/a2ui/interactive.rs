//! Interactive A2UI blocks — buttons, forms, and the callback dispatcher.

use crate::components::icons::*;
use leptos::prelude::*;

use super::blocks::str_field;

// ── Button ──────────────────────────────────────────────────────────

pub fn render_button(data: serde_json::Value) -> impl IntoView {
    let label = str_field(&data, "label").unwrap_or("Click".into());
    let callback_id = str_field(&data, "callback_id").unwrap_or_default();
    let variant = str_field(&data, "variant").unwrap_or("default".into());
    let cls = format!("a2ui-btn a2ui-btn-{}", variant);

    let (clicked, set_clicked) = signal(false);
    let cb_id = callback_id.clone();
    let on_click = move |_: web_sys::MouseEvent| {
        set_clicked.set(true);
        fire_a2ui_callback(&cb_id, serde_json::Value::Null);
    };

    view! {
        <button
            class=cls
            on:click=on_click
            disabled=move || clicked.get()
        >
            {move || if clicked.get() {
                view! { <span class="a2ui-btn-done"><IconCheck size=12 />" Sent"</span> }.into_any()
            } else {
                view! { <span>{label.clone()}</span> }.into_any()
            }}
        </button>
    }
}

// ── Form ────────────────────────────────────────────────────────────

#[derive(Clone)]
struct FormField {
    name: String,
    label: String,
    field_type: String,
    placeholder: String,
    default: String,
}

fn parse_form_fields(data: &serde_json::Value) -> Vec<FormField> {
    data.get("fields")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|f| {
                    Some(FormField {
                        name: f.get("name")?.as_str()?.to_string(),
                        label: f
                            .get("label")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        field_type: f
                            .get("type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("text")
                            .to_string(),
                        placeholder: f
                            .get("placeholder")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        default: f
                            .get("default")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn render_form(data: serde_json::Value) -> impl IntoView {
    let fields = parse_form_fields(&data);
    let callback_id = str_field(&data, "callback_id").unwrap_or_default();
    let submit_label = str_field(&data, "submit_label").unwrap_or("Submit".into());

    let field_signals: Vec<(FormField, RwSignal<String>)> = fields
        .into_iter()
        .map(|f| {
            let default = f.default.clone();
            (f, RwSignal::new(default))
        })
        .collect();

    let (submitted, set_submitted) = signal(false);
    let fields_for_submit = field_signals.clone();
    let cb_id = callback_id.clone();
    let on_submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        set_submitted.set(true);
        let mut values = serde_json::Map::new();
        for (field, sig) in &fields_for_submit {
            values.insert(
                field.name.clone(),
                serde_json::Value::String(sig.get_untracked()),
            );
        }
        fire_a2ui_callback(&cb_id, serde_json::Value::Object(values));
    };

    view! {
        <form class="a2ui-form" on:submit=on_submit>
            {field_signals.iter().map(|(field, sig)| {
                let sig = *sig;
                let name = field.name.clone();
                let label = field.label.clone();
                let placeholder = field.placeholder.clone();
                let ft = field.field_type.clone();
                view! {
                    <div class="a2ui-form-field">
                        <label class="a2ui-form-label">{label}</label>
                        {if ft == "textarea" {
                            view! {
                                <textarea
                                    class="a2ui-form-input"
                                    name=name
                                    placeholder=placeholder
                                    prop:value=move || sig.get()
                                    on:input=move |ev| {
                                        sig.set(leptos::prelude::event_target_value(&ev));
                                    }
                                    disabled=move || submitted.get()
                                />
                            }.into_any()
                        } else {
                            view! {
                                <input
                                    class="a2ui-form-input"
                                    type=ft
                                    name=name
                                    placeholder=placeholder
                                    prop:value=move || sig.get()
                                    on:input=move |ev| {
                                        sig.set(leptos::prelude::event_target_value(&ev));
                                    }
                                    disabled=move || submitted.get()
                                />
                            }.into_any()
                        }}
                    </div>
                }
            }).collect_view()}
            <button
                type="submit"
                class="a2ui-btn a2ui-btn-primary"
                disabled=move || submitted.get()
            >
                {move || if submitted.get() {
                    view! { <span class="a2ui-btn-done"><IconCheck size=12 />" Submitted"</span> }.into_any()
                } else {
                    view! { <span>{submit_label.clone()}</span> }.into_any()
                }}
            </button>
        </form>
    }
}

// ── Callback dispatcher ─────────────────────────────────────────────

/// Fire a custom browser event for A2UI callback. The app layer listens
/// for this and POSTs the callback to the backend.
///
/// We encode the detail as a JSON string and use inline JS to construct
/// the CustomEvent (web_sys does not expose `CustomEventInit` unless the
/// feature is explicitly enabled, and keeping deps small is preferred).
pub fn fire_a2ui_callback(callback_id: &str, payload: serde_json::Value) {
    if callback_id.is_empty() {
        return;
    }
    let detail = serde_json::json!({
        "callback_id": callback_id,
        "payload": payload,
    });
    let detail_str = serde_json::to_string(&detail).unwrap_or_default();

    let _ = web_sys::window().map(|win| {
        // Build CustomEvent via JS so we can set `detail` without CustomEventInit.
        let js = format!(
            "new CustomEvent('opman:a2ui-callback', {{ detail: {} }})",
            // detail_str is already valid JSON; pass it as a JS string literal
            serde_json::to_string(&detail_str).unwrap_or_default()
        );
        if let Ok(evt) = js_sys::eval(&js) {
            use wasm_bindgen::JsCast;
            if let Ok(ce) = evt.dyn_into::<web_sys::CustomEvent>() {
                let _ = win.dispatch_event(&ce);
            }
        }
    });
}
