use wasm_bindgen::JsCast;

mod api;
mod app;
mod components;
mod hooks;
mod pages;
mod sse;
mod theme;
mod types;

fn main() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Debug);

    let root = leptos::tachys::dom::document()
        .get_element_by_id("leptos-root")
        .expect("no #leptos-root element")
        .unchecked_into::<web_sys::HtmlElement>();

    leptos::mount::mount_to(root, app::App).forget();
}
