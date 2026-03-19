//! Root application component with auth check, bootstrap, theme init, and router.
//! Matches React `App.tsx` behavior.

use leptos::prelude::*;
use leptos_router::components::*;
use leptos_router::path;

use crate::components::app_loader::AppLoader;
use crate::components::chat_layout::ChatLayout;
use crate::components::debug_overlay::DebugLog;
use crate::components::error_boundary::ErrorBoundary;
use crate::components::toast::{provide_toast_context, ToastContainer};
use crate::pages::login::LoginPage;

/// Auth state for the application.
#[derive(Clone, Debug, PartialEq)]
enum AuthState {
    Checking,
    Authenticated,
    NotAuthenticated,
}

/// Root application component.
#[component]
pub fn App() -> impl IntoView {
    // Provide toast context at the root
    let _toast_ctx = provide_toast_context();

    // Provide debug overlay context at root (above everything)
    let debug_log = DebugLog::new();
    provide_context(debug_log);

    let (auth_state, set_auth_state) = signal(AuthState::Checking);
    let (bootstrap_ready, set_bootstrap_ready) = signal(false);
    let (app_name, set_app_name) = signal("opman".to_string());

    // Initialize theme mode from localStorage on mount
    crate::theme::init_theme_mode();

    // Run bootstrap + auth check
    leptos::task::spawn_local(async move {
        // Bootstrap (public, no auth required)
        let bootstrap_done = async {
            match crate::api::fetch_bootstrap().await {
                Ok(data) => {
                    if let Some(ref theme) = data.theme {
                        crate::theme::apply_theme_to_css(theme);
                    }
                    if let Some(ref name) = data.instance_name {
                        set_app_name.set(name.clone());
                    }
                }
                Err(e) => {
                    log::warn!("Bootstrap fetch failed: {}", e);
                }
            }
        };

        // Auth check
        let auth_done = async {
            let ok = crate::api::verify_token().await;
            set_auth_state.set(if ok {
                AuthState::Authenticated
            } else {
                AuthState::NotAuthenticated
            });
        };

        // Run both concurrently
        futures::join!(bootstrap_done, auth_done);
        set_bootstrap_ready.set(true);
    });

    let handle_login = Callback::new(move |(_u, _p): (String, String)| {
        set_auth_state.set(AuthState::Authenticated);
    });

    view! {
        <Router>
            <ToastContainer />
            {move || {
                if !bootstrap_ready.get() || auth_state.get() == AuthState::Checking {
                    view! { <AppLoader /> }.into_any()
                } else if auth_state.get() == AuthState::NotAuthenticated {
                    view! { <LoginPage on_login=handle_login app_name=app_name.get() /> }.into_any()
                } else {
                    view! {
                        <ErrorBoundary>
                            <Routes fallback=|| view! { <div class="p-4 text-text-muted">"Page not found"</div> }>
                                <Route path=path!("/") view=MainView />
                                <Route path=path!("/*any") view=MainView />
                            </Routes>
                        </ErrorBoundary>
                    }.into_any()
                }
            }}
        </Router>
    }
}

/// Main authenticated view — wired to ChatLayout.
#[component]
fn MainView() -> impl IntoView {
    view! {
        <ChatLayout />
    }
}
