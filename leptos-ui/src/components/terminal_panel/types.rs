//! Types, constants, and helpers for the terminal panel.
//! Matches React `terminal-panel/types.ts`.

// ── PTY kind constants ─────────────────────────────────────────────

pub const ALL_PTY_KINDS: &[&str] = &["shell", "neovim", "git", "opencode"];

pub fn kind_label(kind: &str) -> &'static str {
    match kind {
        "shell" => "Shell",
        "neovim" => "Neovim",
        "git" => "Git",
        "opencode" => "OpenCode",
        _ => "Shell",
    }
}

// ── Tab model ──────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum TabStatus {
    Connecting,
    Ready,
    Error,
}

#[derive(Clone, Debug)]
pub struct TabInfo {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub status: TabStatus,
}

// ── UUID helper ────────────────────────────────────────────────────

pub fn make_uuid() -> String {
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    let window = web_sys::window().unwrap();
    if let Ok(crypto) = js_sys::Reflect::get(&window, &"crypto".into()) {
        if !crypto.is_undefined() {
            if let Ok(func) = js_sys::Reflect::get(&crypto, &"randomUUID".into()) {
                if func.is_function() {
                    if let Ok(val) = func.unchecked_ref::<js_sys::Function>().call0(&crypto) {
                        if let Some(s) = val.as_string() {
                            return s;
                        }
                    }
                }
            }
        }
    }
    format!(
        "{}-{}",
        js_sys::Date::now() as u64,
        (js_sys::Math::random() * 1e10) as u64
    )
}

// ── Base64 encoding/decoding ───────────────────────────────────────

/// Base64-encode user input for the pty_write API.
pub fn encode_input_b64(input: &str) -> String {
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    let bytes = input.as_bytes();
    let mut binary = String::with_capacity(bytes.len());
    for &b in bytes {
        binary.push(b as char);
    }
    let window = web_sys::window().unwrap();
    js_sys::Reflect::get(&window, &"btoa".into())
        .ok()
        .and_then(|f| f.dyn_into::<js_sys::Function>().ok())
        .and_then(|f| f.call1(&JsValue::NULL, &JsValue::from_str(&binary)).ok())
        .and_then(|v| v.as_string())
        .unwrap_or_default()
}

/// Decode base64 PTY output into raw bytes.
pub fn decode_output_b64(data: &str) -> Vec<u8> {
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    let window = web_sys::window().unwrap();
    let atob = js_sys::Reflect::get(&window, &"atob".into())
        .ok()
        .and_then(|f| f.dyn_into::<js_sys::Function>().ok());
    if let Some(atob_fn) = atob {
        if let Ok(raw) = atob_fn.call1(&JsValue::NULL, &JsValue::from_str(data)) {
            if let Some(raw_str) = raw.as_string() {
                return raw_str.bytes().collect();
            }
        }
    }
    data.as_bytes().to_vec()
}
