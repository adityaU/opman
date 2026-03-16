//! Login page component — matches React `LoginPage.tsx`.

use leptos::prelude::*;

#[component]
pub fn LoginPage(
    #[prop(into)] on_login: Callback<(String, String)>,
    #[prop(default = "opman".to_string(), into)] app_name: String,
) -> impl IntoView {
    let (username, set_username) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (error, set_error) = signal(String::new());
    let (loading, set_loading) = signal(false);
    let (focused, set_focused) = signal(Option::<&'static str>::None);

    let can_submit = Memo::new(move |_| {
        !loading.get() && !username.get().is_empty() && !password.get().is_empty()
    });

    let handle_submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        set_error.set(String::new());
        set_loading.set(true);
        let u = username.get_untracked();
        let p = password.get_untracked();

        leptos::task::spawn_local(async move {
            match crate::api::login(&u, &p).await {
                Ok(_) => {
                    on_login.run((u, p));
                }
                Err(_) => {
                    set_error.set("Invalid username or password".into());
                    set_loading.set(false);
                }
            }
        });
    };

    view! {
        <div class="login-container flex items-center justify-center w-full h-full bg-bg p-4 relative overflow-hidden">
            // Animated background grid
            <div class="absolute inset-0 pointer-events-none opacity-30"
                style="background-image: linear-gradient(rgba(255,255,255,0.02) 1px, transparent 1px), linear-gradient(90deg, rgba(255,255,255,0.02) 1px, transparent 1px); background-size: 40px 40px; animation: login-grid-drift 20s linear infinite;" />

            // Radial glow
            <div class="absolute w-[500px] h-[500px] rounded-full pointer-events-none top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2"
                style="background: radial-gradient(circle, color-mix(in srgb, var(--color-primary) 12%, transparent) 0%, transparent 70%); animation: login-glow-pulse 4s ease-in-out infinite;" />

            <form
                class="w-full max-w-[400px] p-8 rounded-[28px] liquid-glass-floating relative z-10"
                on:submit=handle_submit
            >
                // Brand
                <div class="text-center mb-8">
                    <div class="inline-flex items-center justify-center w-[52px] h-[52px] rounded-[16px] mb-3"
                        style="background: color-mix(in srgb, var(--color-primary) 15%, var(--color-bg-element)); border: 1px solid color-mix(in srgb, var(--color-primary) 30%, transparent);">
                        <svg class="text-primary" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                            <polyline points="4 17 10 11 4 5"/>
                            <line x1="12" y1="19" x2="20" y2="19"/>
                        </svg>
                    </div>
                    <h1 class="text-xl font-bold text-primary mb-1 tracking-wider">{app_name.clone()}</h1>
                    <div class="text-xs text-text-muted">"AI-Powered Development Environment"</div>
                </div>

                // Username field
                <div class="mb-4">
                    <label class=move || {
                        let base = "flex items-center gap-[5px] text-2xs uppercase tracking-wider font-medium mb-1 transition-colors";
                        if focused.get() == Some("user") { format!("{base} text-primary") } else { format!("{base} text-text-muted") }
                    }>
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/>
                            <circle cx="12" cy="7" r="4"/>
                        </svg>
                        "Username"
                    </label>
                    <input
                        type="text"
                        class="w-full p-3 rounded-lg text-sm text-text outline-none transition-all"
                        style="background: rgba(0,0,0,0.2); border: 0.5px solid rgba(255,255,255,0.06);"
                        prop:value=move || username.get()
                        on:input=move |ev| set_username.set(event_target_value(&ev))
                        on:focus=move |_| set_focused.set(Some("user"))
                        on:blur=move |_| set_focused.set(None)
                        autofocus=true
                        autocomplete="username"
                        placeholder="Enter username"
                    />
                </div>

                // Password field
                <div class="mb-4">
                    <label class=move || {
                        let base = "flex items-center gap-[5px] text-2xs uppercase tracking-wider font-medium mb-1 transition-colors";
                        if focused.get() == Some("pass") { format!("{base} text-primary") } else { format!("{base} text-text-muted") }
                    }>
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/>
                            <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
                        </svg>
                        "Password"
                    </label>
                    <input
                        type="password"
                        class="w-full p-3 rounded-lg text-sm text-text outline-none transition-all"
                        style="background: rgba(0,0,0,0.2); border: 0.5px solid rgba(255,255,255,0.06);"
                        prop:value=move || password.get()
                        on:input=move |ev| set_password.set(event_target_value(&ev))
                        on:focus=move |_| set_focused.set(Some("pass"))
                        on:blur=move |_| set_focused.set(None)
                        autocomplete="current-password"
                        placeholder="Enter password"
                    />
                </div>

                // Submit button
                <button
                    type="submit"
                    class="w-full p-3 mt-3 bg-primary border-none rounded-full text-bg text-sm font-semibold cursor-pointer flex items-center justify-center gap-[6px] transition-all hover:opacity-90 active:scale-[0.97] disabled:opacity-40 disabled:cursor-not-allowed"
                    disabled=move || !can_submit.get()
                >
                    {move || if loading.get() {
                        view! {
                            <svg class="animate-spin" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                <path d="M21 12a9 9 0 1 1-6.219-8.56"/>
                            </svg>
                            <span>"Authenticating..."</span>
                        }.into_any()
                    } else {
                        view! {
                            <span>"Sign In"</span>
                            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                <line x1="5" y1="12" x2="19" y2="12"/>
                                <polyline points="12 5 19 12 12 19"/>
                            </svg>
                        }.into_any()
                    }}
                </button>

                // Error
                <Show when=move || !error.get().is_empty()>
                    <div class="login-error text-error text-xs text-center mt-3">{move || error.get()}</div>
                </Show>

                // Footer
                <div class="text-center mt-4 text-[11px] text-text-muted">
                    <kbd class="bg-bg-element border border-border-subtle rounded px-[5px] py-[1px] text-[10px] font-mono">"Enter"</kbd>
                    " to sign in"
                </div>
            </form>
        </div>
    }
}
