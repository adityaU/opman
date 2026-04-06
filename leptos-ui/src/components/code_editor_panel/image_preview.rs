//! Image preview with zoom, pan, and drag support.
//! Uses CSS transforms for smooth interaction.

use leptos::prelude::*;

/// Render an image preview with scroll-to-zoom, click-drag-to-pan controls.
pub fn render_image_preview(url: &str, alt: &str) -> leptos::tachys::view::any_view::AnyView {
    let url = url.to_string();
    let alt = alt.to_string();

    let (scale, set_scale) = signal(1.0_f64);
    let (tx, set_tx) = signal(0.0_f64);
    let (ty, set_ty) = signal(0.0_f64);
    let (dragging, set_dragging) = signal(false);
    let (drag_start_x, set_drag_start_x) = signal(0.0_f64);
    let (drag_start_y, set_drag_start_y) = signal(0.0_f64);
    let (tx_start, set_tx_start) = signal(0.0_f64);
    let (ty_start, set_ty_start) = signal(0.0_f64);

    let transform = Memo::new(move |_| {
        format!(
            "translate({}px, {}px) scale({})",
            tx.get(),
            ty.get(),
            scale.get()
        )
    });

    let on_wheel = move |ev: web_sys::WheelEvent| {
        ev.prevent_default();
        let delta = ev.delta_y();
        let factor = if delta < 0.0 { 1.1 } else { 1.0 / 1.1 };
        let new_scale = (scale.get_untracked() * factor).clamp(0.1, 20.0);
        set_scale.set(new_scale);
    };

    let on_mousedown = move |ev: web_sys::MouseEvent| {
        if ev.button() != 0 {
            return;
        }
        ev.prevent_default();
        set_dragging.set(true);
        set_drag_start_x.set(ev.client_x() as f64);
        set_drag_start_y.set(ev.client_y() as f64);
        set_tx_start.set(tx.get_untracked());
        set_ty_start.set(ty.get_untracked());
    };

    let on_mousemove = move |ev: web_sys::MouseEvent| {
        if !dragging.get_untracked() {
            return;
        }
        let dx = ev.client_x() as f64 - drag_start_x.get_untracked();
        let dy = ev.client_y() as f64 - drag_start_y.get_untracked();
        set_tx.set(tx_start.get_untracked() + dx);
        set_ty.set(ty_start.get_untracked() + dy);
    };

    let on_mouseup = move |_ev: web_sys::MouseEvent| {
        set_dragging.set(false);
    };

    let on_dblclick = move |_ev: web_sys::MouseEvent| {
        set_scale.set(1.0);
        set_tx.set(0.0);
        set_ty.set(0.0);
    };

    let scale_pct = Memo::new(move |_| format!("{}%", (scale.get() * 100.0).round() as i32));

    let on_zoom_in = move |_| {
        set_scale.set((scale.get_untracked() * 1.25).clamp(0.1, 20.0));
    };
    let on_zoom_out = move |_| {
        set_scale.set((scale.get_untracked() / 1.25).clamp(0.1, 20.0));
    };
    let on_reset = move |_| {
        set_scale.set(1.0);
        set_tx.set(0.0);
        set_ty.set(0.0);
    };

    view! {
        <div class="file-preview file-preview-image-zoom">
            <div class="image-zoom-toolbar">
                <button class="image-zoom-btn" on:click=on_zoom_out title="Zoom out">"\u{2212}"</button>
                <span class="image-zoom-level">{scale_pct}</span>
                <button class="image-zoom-btn" on:click=on_zoom_in title="Zoom in">"+"</button>
                <button class="image-zoom-btn image-zoom-reset" on:click=on_reset title="Reset (or double-click)">"\u{21BA}"</button>
            </div>
            <div
                class="image-zoom-canvas"
                on:wheel=on_wheel
                on:mousedown=on_mousedown
                on:mousemove=on_mousemove
                on:mouseup=on_mouseup
                on:mouseleave=move |_| set_dragging.set(false)
                on:dblclick=on_dblclick
                style:cursor=move || if dragging.get() { "grabbing" } else { "grab" }
            >
                <img
                    src=url
                    alt=alt
                    draggable="false"
                    style:transform=transform
                    style:transform-origin="center center"
                />
            </div>
            <div class="image-zoom-hint">"Scroll to zoom \u{2022} Drag to pan \u{2022} Double-click to reset"</div>
        </div>
    }
    .into_any()
}
