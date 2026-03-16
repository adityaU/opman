//! CodeBlock — fenced code block with language label, line numbers, copy, download, word-wrap toggle.
//! Leptos port of `web-ui/src/message-turn/CodeBlock.tsx`.
//!
//! Syntax highlighting is deferred — we render with a `language-xxx` class on the `<code>` tag
//! so a client-side highlighter (e.g. highlight.js loaded from CDN) can decorate after mount.
//! This keeps the WASM binary small while still supporting highlighting.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use std::collections::HashMap;

use super::syntax_highlight::highlight_code;

// ── Constants ───────────────────────────────────────────────────────

/// File extension mapping for common languages (for download filename).
pub fn lang_extensions() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("javascript", "js");
    m.insert("typescript", "ts");
    m.insert("python", "py");
    m.insert("rust", "rs");
    m.insert("ruby", "rb");
    m.insert("go", "go");
    m.insert("java", "java");
    m.insert("kotlin", "kt");
    m.insert("swift", "swift");
    m.insert("csharp", "cs");
    m.insert("cpp", "cpp");
    m.insert("c", "c");
    m.insert("html", "html");
    m.insert("css", "css");
    m.insert("json", "json");
    m.insert("yaml", "yml");
    m.insert("toml", "toml");
    m.insert("markdown", "md");
    m.insert("bash", "sh");
    m.insert("shell", "sh");
    m.insert("sql", "sql");
    m.insert("xml", "xml");
    m.insert("jsx", "jsx");
    m.insert("tsx", "tsx");
    m.insert("php", "php");
    m.insert("lua", "lua");
    m.insert("zig", "zig");
    m
}

/// Extension-to-language mapping for guessing language from file path.
pub fn ext_to_lang(ext: &str) -> &'static str {
    match ext {
        "ts" => "typescript",
        "tsx" => "tsx",
        "js" => "javascript",
        "jsx" => "jsx",
        "py" => "python",
        "rs" => "rust",
        "go" => "go",
        "rb" => "ruby",
        "java" => "java",
        "kt" => "kotlin",
        "swift" => "swift",
        "c" => "c",
        "cpp" | "cc" | "cxx" => "cpp",
        "h" => "c",
        "hpp" => "cpp",
        "cs" => "csharp",
        "css" => "css",
        "scss" => "scss",
        "html" => "html",
        "xml" => "xml",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "md" => "markdown",
        "sh" | "bash" | "zsh" => "bash",
        "sql" => "sql",
        "lua" => "lua",
        "vim" => "vim",
        "dockerfile" => "dockerfile",
        "makefile" => "makefile",
        _ => "text",
    }
}

/// Guess syntax language from a file path.
pub fn guess_language(path: &str) -> &'static str {
    let filename = path.rsplit('/').next().unwrap_or("");
    let lower = filename.to_lowercase();

    if lower == "dockerfile" {
        return "dockerfile";
    }
    if lower == "makefile" || lower == "gnumakefile" {
        return "makefile";
    }
    if lower.ends_with(".lock") {
        return "json";
    }

    let ext = lower.rsplit('.').next().unwrap_or("");
    ext_to_lang(ext)
}

/// Interactive code block with language label, line numbers, copy, download, word-wrap toggle.
#[component]
pub fn CodeBlock(
    #[prop(into)] language: String,
    #[prop(into)] code: String,
) -> impl IntoView {
    let (copied, set_copied) = signal(false);
    let (word_wrap, set_word_wrap) = signal(true);

    let code_for_copy = code.clone();
    let code_for_download = code.clone();
    let lang_for_download = language.clone();
    let lang_for_class = language.clone();

    // Pre-highlight the code into HTML with CSS token classes
    let highlighted_html = highlight_code(&code, &language);

    let line_count = code.lines().count().max(1);

    let handle_copy = move |_: web_sys::MouseEvent| {
        let code_val = code_for_copy.clone();
        let set_c = set_copied;
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(window) = web_sys::window() {
                let clipboard = window.navigator().clipboard();
                let _ = wasm_bindgen_futures::JsFuture::from(
                    clipboard.write_text(&code_val),
                )
                .await;
                set_c.set(true);
                gloo_timers::future::TimeoutFuture::new(1500).await;
                set_c.set(false);
            }
        });
    };

    let handle_download = move |_: web_sys::MouseEvent| {
        let ext = lang_extensions()
            .get(lang_for_download.as_str())
            .copied()
            .unwrap_or("txt");
        let code_val = code_for_download.clone();

        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            // Create blob URL and trigger download
            let blob_parts = js_sys::Array::new();
            blob_parts.push(&JsValue::from_str(&code_val));
            if let Ok(blob) = web_sys::Blob::new_with_str_sequence(&blob_parts) {
                if let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) {
                    if let Ok(a) = document.create_element("a") {
                        let _ = a.set_attribute("href", &url);
                        let _ = a.set_attribute("download", &format!("snippet.{}", ext));
                        if let Some(body) = document.body() {
                            let _ = body.append_child(&a);
                            if let Some(html_a) = a.dyn_ref::<web_sys::HtmlElement>() {
                                html_a.click();
                            }
                            let _ = body.remove_child(&a);
                        }
                        let _ = web_sys::Url::revoke_object_url(&url);
                    }
                }
            }
        }
    };

    let toggle_wrap = move |_: web_sys::MouseEvent| {
        set_word_wrap.update(|v| *v = !*v);
    };

    view! {
        <div class=move || {
            if word_wrap.get() {
                "code-block-wrapper rounded-lg border border-border-subtle overflow-hidden my-2 bg-bg/60"
            } else {
                "code-block-wrapper code-block-nowrap rounded-lg border border-border-subtle overflow-hidden my-2 bg-bg/60"
            }
        }>
            // Header bar
            <div class="code-block-header flex items-center justify-between px-3 py-1.5 bg-bg-panel/60 border-b border-border-subtle">
                <span class="text-[10px] font-mono text-text-muted uppercase tracking-wider">{language}</span>
                <div class="code-block-actions flex items-center gap-1">
                    // Word wrap toggle
                    <button
                        class=move || {
                            if word_wrap.get() {
                                "code-block-action-btn p-1 rounded text-text-muted hover:text-text hover:bg-bg-element/50 transition-colors text-primary"
                            } else {
                                "code-block-action-btn p-1 rounded text-text-muted hover:text-text hover:bg-bg-element/50 transition-colors"
                            }
                        }
                        on:click=toggle_wrap
                        title="Toggle word wrap"
                    >
                        <svg class="w-3 h-3" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
                            <path d="M3 4h10M3 8h7a2 2 0 010 4H8l1.5-1.5M3 12h4" />
                        </svg>
                    </button>
                    // Download
                    <button
                        class="code-block-action-btn p-1 rounded text-text-muted hover:text-text hover:bg-bg-element/50 transition-colors"
                        on:click=handle_download
                        title="Download"
                    >
                        <svg class="w-3 h-3" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
                            <path d="M8 2v8m0 0l-3-3m3 3l3-3M3 12h10" />
                        </svg>
                    </button>
                    // Copy
                    <button
                        class="code-block-action-btn p-1 rounded text-text-muted hover:text-text hover:bg-bg-element/50 transition-colors"
                        on:click=handle_copy
                        title="Copy"
                        aria-label="Copy code"
                    >
                        {move || if copied.get() {
                            view! {
                                <svg class="w-3 h-3 text-success" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2">
                                    <path d="M3 8l3 3 7-7" />
                                </svg>
                            }.into_any()
                        } else {
                            view! {
                                <svg class="w-3 h-3" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
                                    <rect x="5" y="5" width="8" height="8" rx="1" />
                                    <path d="M3 11V3h8" />
                                </svg>
                            }.into_any()
                        }}
                    </button>
                </div>
            </div>
            // Code body
            <div class="code-block-body flex overflow-x-auto">
                // Line numbers
                <div
                    class="code-block-line-numbers flex flex-col items-end pr-3 pl-3 py-2 text-[11px] font-mono text-text-muted/40 select-none border-r border-border-subtle/50 bg-bg-panel/30"
                    aria-hidden="true"
                >
                    {(1..=line_count).map(|n| view! { <span class="leading-[1.6]">{n}</span> }).collect_view()}
                </div>
                // Code content
                <pre
                    class=move || {
                        if word_wrap.get() {
                            "flex-1 min-w-0 py-2 pl-3 pr-3 text-[12px] font-mono leading-[1.6] whitespace-pre-wrap break-words overflow-hidden"
                        } else {
                            "flex-1 min-w-0 py-2 pl-3 pr-3 text-[12px] font-mono leading-[1.6] whitespace-pre overflow-x-auto"
                        }
                    }
                >
                    <code class=format!("language-{}", lang_for_class) inner_html=highlighted_html></code>
                </pre>
            </div>
        </div>
    }
}

/// Render inline code (not fenced).
#[component]
pub fn InlineCode(
    #[prop(into)] children: String,
) -> impl IntoView {
    view! {
        <code class="inline-code px-1.5 py-0.5 rounded bg-bg-panel/80 border border-border-subtle text-[0.85em] font-mono text-primary/80">
            {children}
        </code>
    }
}
