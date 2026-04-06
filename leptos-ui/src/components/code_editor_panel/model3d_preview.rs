//! Leptos component for 3D model preview using Three.js.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use super::cad_viewer;

/// Render a 3D model preview with orbit controls (zoom/pan/rotate).
pub fn render_model3d_view(path: &str) -> leptos::tachys::view::any_view::AnyView {
    let url = crate::api::files::file_raw_url(path);
    let ext = path.rsplit('.').next().unwrap_or("stl").to_lowercase();
    let container_id = format!(
        "cad-viewer-{}",
        js_sys::Math::random().to_bits() & 0xFFFFFFFF
    );
    let cid = container_id.clone();
    let cid_cleanup = container_id.clone();

    // Mount effect: init Three.js scene after DOM element exists
    let url_c = url.clone();
    let ext_c = ext.clone();
    let cid_mount = container_id.clone();
    Effect::new(move |_| {
        let cid = cid_mount.clone();
        let url = url_c.clone();
        let ext = ext_c.clone();
        // Delay slightly to ensure the container element is in the DOM
        let cb = wasm_bindgen::closure::Closure::once(move || {
            cad_viewer::init_3d_viewer(&cid, &url, &ext);
        });
        let _ = web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(cb.as_ref().unchecked_ref(), 50);
        cb.forget();
    });

    // Cleanup on unmount
    on_cleanup(move || {
        cad_viewer::dispose_3d_viewer(&cid_cleanup);
    });

    let name = path.rsplit('/').next().unwrap_or(path).to_string();
    let ext_upper = ext.to_uppercase();

    view! {
        <div class="file-preview file-preview-cad">
            <div class="cad-header">
                <span class="cad-file-name">{name}</span>
                <span class="cad-format-badge">{ext_upper}</span>
                <span class="cad-hint">"Scroll to zoom \u{2022} Drag to rotate \u{2022} Right-click to pan"</span>
            </div>
            <div class="cad-canvas-container" id=cid></div>
            <div class="cad-loading">"Loading 3D model\u{2026}"</div>
        </div>
    }
    .into_any()
}
