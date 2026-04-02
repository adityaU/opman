//! MessageTurn — renders a group of consecutive same-role messages.
//! Leptos port of `web-ui/src/message-turn/MessageTurn.tsx`.
//!
//! Features:
//!   - grouped role rendering with avatar + role label
//!   - Markdown rendering via pulldown-cmark (matches React's ReactMarkdown + remarkGfm)
//!   - fenced code blocks rendered via CodeBlock component
//!   - interleaved tool call rendering between text blocks
//!   - model / agent / cost header chips
//!   - error banner
//!   - optimistic / queued badges
//!   - copy / retry / bookmark action bar
//!   - search-match highlighting

use leptos::prelude::*;
use leptos::callback::Callable;
use crate::types::core::{Message, MessagePart};
use crate::types::api::SessionInfo;
use crate::components::tool_call::{ToolCallView, ChildSessionRef, SubagentMessagesMap};
use crate::components::code_block::CodeBlock;
use crate::components::icons::*;
use std::collections::{HashMap, HashSet};

// ── Message grouping ────────────────────────────────────────────────

/// A group of consecutive messages sharing the same role.
#[derive(Debug, Clone, PartialEq)]
pub struct MessageGroup {
    pub role: String,
    pub messages: Vec<Message>,
    pub key: String,
}

/// Group consecutive same-role messages together.
pub fn group_messages(messages: &[Message]) -> Vec<MessageGroup> {
    let mut groups: Vec<MessageGroup> = Vec::new();
    for msg in messages {
        let last = groups.last_mut();
        if let Some(last) = last {
            if last.role == msg.info.role {
                last.messages.push(msg.clone());
                continue;
            }
        }
        groups.push(MessageGroup {
            role: msg.info.role.clone(),
            messages: vec![msg.clone()],
            key: msg.info.effective_id(),
        });
    }
    groups
}

// ── Agent color helper ──────────────────────────────────────────────

/// Simple djb2 hash for deterministic agent colors.
fn hash_string(s: &str) -> u32 {
    let mut h: u32 = 5381;
    for b in s.to_lowercase().bytes() {
        h = h.wrapping_shl(5).wrapping_add(h).wrapping_add(b as u32);
    }
    h
}

const AGENT_PALETTE: &[&str] = &[
    "var(--color-primary)",
    "var(--color-secondary)",
    "var(--color-accent)",
    "var(--color-info)",
    "var(--color-success)",
    "var(--color-warning)",
    "var(--color-error)",
];

fn agent_color(id: &str) -> &'static str {
    let idx = hash_string(id) as usize % AGENT_PALETTE.len();
    AGENT_PALETTE[idx]
}

/// Get model display label.
fn model_label(model: &serde_json::Value) -> String {
    match model {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(m) => {
            m.get("modelID")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| {
                    m.get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                })
                .to_string()
        }
        _ => format!("{}", model),
    }
}

// ── Memory guidance parsing ─────────────────────────────────────────

/// Parsed user message: separates memory guidance from the actual request.
struct ParsedUserMessage {
    user_text: String,
    memory_lines: Option<String>,
}

/// Parse a user message that may contain injected memory guidance.
/// Pattern: `[Assistant memory in effect]\n...\n\n[User request]\n...`
fn parse_memory_guidance(text: &str) -> ParsedUserMessage {
    let trimmed = text.trim();
    if !trimmed.starts_with("[Assistant memory in effect]") {
        return ParsedUserMessage { user_text: trimmed.to_string(), memory_lines: None };
    }
    if let Some(req_pos) = trimmed.find("[User request]") {
        let mem_block = trimmed["[Assistant memory in effect]".len()..req_pos].trim();
        let user_block = trimmed[req_pos + "[User request]".len()..].trim();
        ParsedUserMessage {
            user_text: user_block.to_string(),
            memory_lines: if mem_block.is_empty() { None } else { Some(mem_block.to_string()) },
        }
    } else {
        ParsedUserMessage { user_text: trimmed.to_string(), memory_lines: None }
    }
}

// ── Markdown rendering ──────────────────────────────────────────────

/// A segment of content — either rendered HTML (from markdown) or a fenced code block.
#[derive(Clone)]
pub enum ContentSegment {
    Html(String),
    FencedCode { language: String, code: String },
}

// ── Markdown cache ──────────────────────────────────────────────────
// WASM is single-threaded so thread_local + RefCell is safe and zero-cost.
// We cache parsed segments keyed by the full text content.
// During streaming, only the *last* message's text changes, so earlier
// groups' cached results stay valid. The cache is bounded to prevent
// memory bloat: when it exceeds MAX_CACHE_ENTRIES we clear it entirely
// (simple but effective since streaming keeps invalidating the latest entry).

use std::cell::RefCell;

const MAX_MARKDOWN_CACHE_ENTRIES: usize = 128;

thread_local! {
    static MARKDOWN_CACHE: RefCell<HashMap<u64, Vec<ContentSegment>>> =
        RefCell::new(HashMap::with_capacity(64));
}

/// Fast hash for cache keying (FxHash-style, good enough for content dedup).
fn hash_text(text: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325; // FNV offset basis
    for byte in text.bytes() {
        h ^= byte as u64;
        h = h.wrapping_mul(0x100000001b3); // FNV prime
    }
    h
}

/// Parse markdown text into segments, with caching.
/// Cache hits are O(1) lookups; misses run pulldown-cmark and store the result.
pub fn parse_markdown_segments(text: &str) -> Vec<ContentSegment> {
    let key = hash_text(text);
    
    // Check cache first
    let cached = MARKDOWN_CACHE.with(|cache| {
        cache.borrow().get(&key).cloned()
    });
    if let Some(segments) = cached {
        return segments;
    }
    
    // Cache miss — parse
    let segments = parse_markdown_segments_uncached(text);
    
    // Store in cache
    MARKDOWN_CACHE.with(|cache| {
        let mut c = cache.borrow_mut();
        if c.len() >= MAX_MARKDOWN_CACHE_ENTRIES {
            c.clear(); // Simple eviction: clear all when full
        }
        c.insert(key, segments.clone());
    });
    
    segments
}

/// Parse markdown text into segments, splitting out fenced code blocks
/// so they can be rendered with our CodeBlock component, and rendering
/// the rest as HTML via pulldown-cmark.
fn parse_markdown_segments_uncached(text: &str) -> Vec<ContentSegment> {
    use pulldown_cmark::{Parser, Event, Tag, TagEnd, Options, CodeBlockKind, CowStr};

    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(text, opts);

    let mut segments: Vec<ContentSegment> = Vec::new();
    let mut current_events: Vec<Event<'_>> = Vec::new();
    let mut in_code_block = false;
    let mut code_block_lang = String::new();
    let mut code_block_content = String::new();

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                // Flush accumulated markdown events as HTML
                if !current_events.is_empty() {
                    let html = events_to_html(&current_events);
                    let trimmed = html.trim();
                    if !trimmed.is_empty() {
                        segments.push(ContentSegment::Html(trimmed.to_string()));
                    }
                    current_events.clear();
                }
                in_code_block = true;
                code_block_lang = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        let l = lang.to_string().trim().to_string();
                        if l.is_empty() { "text".to_string() } else { l }
                    }
                    CodeBlockKind::Indented => "text".to_string(),
                };
                code_block_content.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                segments.push(ContentSegment::FencedCode {
                    language: code_block_lang.clone(),
                    code: code_block_content.clone(),
                });
                code_block_lang.clear();
                code_block_content.clear();
            }
            Event::Text(t) if in_code_block => {
                code_block_content.push_str(&t);
            }
            _ => {
                if !in_code_block {
                    current_events.push(event);
                }
            }
        }
    }

    // Flush remaining events
    if !current_events.is_empty() {
        let html = events_to_html(&current_events);
        let trimmed = html.trim();
        if !trimmed.is_empty() {
            segments.push(ContentSegment::Html(trimmed.to_string()));
        }
    }

    segments
}

/// Convert a slice of pulldown-cmark events to an HTML string.
fn events_to_html(events: &[pulldown_cmark::Event<'_>]) -> String {
    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, events.iter().cloned());
    // Post-process: add class="inline-code" to inline <code> elements
    // (not inside <pre>, which is handled by CodeBlock component)
    html_output = html_output.replace("<code>", "<code class=\"inline-code\">");
    html_output
}

// ── MessageTurn component ───────────────────────────────────────────

#[component]
pub fn MessageTurn(
    group: MessageGroup,
    #[prop(optional)] child_sessions: Option<Vec<SessionInfo>>,
    #[prop(optional)] subagent_messages: Option<ReadSignal<SubagentMessagesMap>>,
    search_match_ids: Option<HashSet<String>>,
    active_search_match_id: Option<String>,
    #[prop(optional)] on_retry: Option<Callback<String>>,
    #[prop(optional)] on_open_session: Option<Callback<String>>,
    session_id: Option<String>,
    #[prop(optional)] pending_assistant_id: Option<String>,
    is_bookmarked: Option<Callback<String, bool>>,
    on_toggle_bookmark: Option<Callback<(String, String, String, String)>>,
) -> impl IntoView {
    let role = group.role.clone();
    let messages = group.messages.clone();
    let is_user = role == "user";
    let is_assistant = role == "assistant";

    // Skip system messages
    if role == "system" {
        return view! { <div /> }.into_any();
    }

    let (copied, set_copied) = signal(false);

    let first_msg_id = messages.first()
        .map(|m| m.info.effective_id())
        .unwrap_or_default();

    // Search match detection
    let is_search_match = search_match_ids.as_ref().map_or(false, |ids| {
        messages.iter().any(|msg| ids.contains(&msg.info.effective_id()))
    });
    let is_active_match = active_search_match_id.as_ref().map_or(false, |active_id| {
        messages.iter().any(|msg| msg.info.effective_id() == *active_id)
    });

    // Bookmark state (React: isBookmarked ? isBookmarked(firstMsgId) : false)
    let bookmarked = is_bookmarked.as_ref().map_or(false, |cb| {
        if first_msg_id.is_empty() { false } else { cb.run(first_msg_id.clone()) }
    });

    // Bookmark handler
    let first_id_for_bookmark = first_msg_id.clone();
    let sid_for_bookmark = session_id.clone().unwrap_or_default();
    let role_for_bookmark = role.clone();
    let plain_for_bookmark = {
        let all_parts_preview: Vec<&str> = messages.iter()
            .flat_map(|m| m.parts.iter())
            .filter(|p| p.part_type == "text")
            .filter_map(|p| p.text.as_deref())
            .collect();
        all_parts_preview.first().copied().unwrap_or("").to_string()
    };
    let handle_toggle_bookmark = {
        let on_toggle = on_toggle_bookmark.clone();
        move |_: web_sys::MouseEvent| {
            if let Some(ref cb) = on_toggle {
                if !first_id_for_bookmark.is_empty() {
                    cb.run((
                        first_id_for_bookmark.clone(),
                        sid_for_bookmark.clone(),
                        role_for_bookmark.clone(),
                        plain_for_bookmark.clone(),
                    ));
                }
            }
        }
    };

    // Optimistic detection
    let is_optimistic = messages.iter().any(|msg| {
        msg.info.effective_id().starts_with("__optimistic__")
    });

    // Queued detection
    let is_queued = if is_user {
        pending_assistant_id.as_ref().map_or(false, |pending_id| {
            messages.iter().any(|msg| msg.info.effective_id() > *pending_id)
        })
    } else {
        false
    };

    // Header metadata
    let header_model = messages.iter().find_map(|m| m.info.model.clone());
    let header_agent = messages.iter().find_map(|m| m.info.agent.clone());
    let total_cost: f64 = messages.iter()
        .filter_map(|m| m.metadata.as_ref().and_then(|md| md.cost))
        .sum();

    // Error
    let error_text = messages.iter().find_map(|m| {
        m.info.error.as_ref().map(|e| {
            match e {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Object(obj) => {
                    obj.get("message")
                        .or_else(|| obj.get("error"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                }
                _ => format!("{}", e),
            }
        })
    }).filter(|s| !s.is_empty());

    // Flatten all parts from all messages
    let all_parts: Vec<MessagePart> = messages.iter()
        .flat_map(|m| m.parts.iter().cloned())
        .collect();

    // Check for tool calls
    let has_tool_calls = all_parts.iter().any(|p| {
        p.part_type == "tool" || p.part_type == "tool-call" || p.part_type == "tool_call"
    });

    // Collect all text
    let all_text: String = all_parts.iter()
        .filter(|p| p.part_type == "text")
        .filter_map(|p| p.text.as_deref())
        .collect::<Vec<_>>()
        .join("\n");

    // For user messages, parse out injected memory guidance
    let parsed = if is_user {
        parse_memory_guidance(&all_text)
    } else {
        ParsedUserMessage { user_text: all_text.trim().to_string(), memory_lines: None }
    };
    let plain_text = parsed.user_text;
    let memory_guidance = parsed.memory_lines;

    // Copy handler
    let plain_for_copy = plain_text.clone();
    let handle_copy = move |_: web_sys::MouseEvent| {
        let text = plain_for_copy.clone();
        let set_c = set_copied;
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(window) = web_sys::window() {
                let clipboard = window.navigator().clipboard();
                let _ = wasm_bindgen_futures::JsFuture::from(
                    clipboard.write_text(&text),
                ).await;
                set_c.set(true);
                gloo_timers::future::TimeoutFuture::new(1500).await;
                set_c.set(false);
            }
        });
    };

    // Retry handler
    let plain_for_retry = plain_text.clone();
    let handle_retry = move |_: web_sys::MouseEvent| {
        if let Some(ref cb) = on_retry {
            cb.run(plain_for_retry.clone());
        }
    };

    // Build wrapper classes (React: message-turn message-turn-{role} [.message-turn-optimistic] [.message-turn-search-match] [.message-turn-active-match])
    let mut wrapper_class = format!("message-turn message-turn-{}", role);
    if is_optimistic { wrapper_class.push_str(" message-turn-optimistic"); }
    if is_search_match { wrapper_class.push_str(" message-turn-search-match"); }
    if is_active_match { wrapper_class.push_str(" message-turn-active-match"); }

    // Build content
    let content_view = if has_tool_calls {
        // Interleaved rendering: text + tool calls
        render_interleaved(
            &all_parts,
            child_sessions.as_deref().unwrap_or(&[]),
            subagent_messages.clone(),
            on_open_session.clone(),
        )
    } else if !plain_text.is_empty() {
        if let Some(ref mem_text) = memory_guidance {
            // User message with memory guidance — render accordion
            render_user_memory_accordion(&plain_text, mem_text)
        } else {
            // Pure text — render as Markdown with fenced code blocks
            render_markdown_body(&plain_text)
        }
    } else {
        view! { <div /> }.into_any()
    };

    let has_plain_text = !plain_text.is_empty();
    let has_retry = is_user && on_retry.is_some() && has_plain_text;

    view! {
        <div class=wrapper_class>
            <div class="message-content">
                // Header (React: div.message-header)
                <div class="message-header">
                    // Avatar (React: div.message-avatar — User/Bot/Wrench)
                    <div class={format!("message-avatar {}", role)}>
                        {if is_user {
                            view! { <IconUser size=16 /> }.into_any()
                        } else if is_assistant {
                            view! { <IconBot size=16 /> }.into_any()
                        } else {
                            view! { <IconWrench size=16 /> }.into_any()
                        }}
                    </div>
                    // Role label (React: span.message-role)
                    <span class="message-role">
                        {if is_user { "You".to_string() } else if is_assistant { "Assistant".to_string() } else { role.clone() }}
                    </span>
                    // Badges
                    {(is_optimistic && !is_queued).then(|| view! {
                        <span class="message-sending-badge">"Sending..."</span>
                    })}
                    {is_queued.then(|| view! {
                        <span class="message-queued-badge">"Queued"</span>
                    })}
                    // Model (React: span.message-model)
                    {header_model.as_ref().map(|m| {
                        let label = model_label(m);
                        view! {
                            <span class="message-model">{label}</span>
                        }
                    })}
                    // Cost (React: span.message-cost)
                    {(total_cost > 0.0).then(|| view! {
                        <span class="message-cost">{format!("${:.4}", total_cost)}</span>
                    })}
                    // Agent (React: isAssistant && headerAgent — only show for assistant)
                    {(is_assistant && header_agent.is_some()).then(|| {
                        let a = header_agent.as_ref().unwrap();
                        let color = agent_color(a);
                        view! {
                            <span
                                class="message-agent"
                                style=format!(
                                    "color: {}; border-color: color-mix(in srgb, {} 25%, transparent); background-color: color-mix(in srgb, {} 10%, transparent);",
                                    color, color, color
                                )
                            >
                                {a.clone()}
                            </span>
                        }
                    })}
                </div>

                // Content
                {content_view}

                // Error banner (React: div.message-error-banner)
                {error_text.map(|e| view! {
                    <div class="message-error-banner">
                        <IconAlertTriangle size=14 />
                        <span>{e}</span>
                    </div>
                })}

                // Action bar (React: div.message-actions, hidden until hover)
                // React order: bookmark, copy, retry, model-label
                {(!is_optimistic).then(|| {
                    let has_bookmark_fn = on_toggle_bookmark.is_some();
                    let has_first_id = !first_msg_id.is_empty();
                    let show_bookmark = has_bookmark_fn && has_first_id;
                    let model_for_bar = if is_assistant { header_model.clone() } else { None };

                    view! {
                        <div class="message-actions">
                            // Bookmark (React: first button)
                            {show_bookmark.then(|| {
                                let bm_class = if bookmarked { "message-action-btn bookmarked" } else { "message-action-btn" };
                                let bm_title = if bookmarked { "Remove bookmark" } else { "Bookmark message" };
                                let bm_label = if bookmarked { "Remove bookmark" } else { "Bookmark message" };
                                view! {
                                    <button
                                        class=bm_class
                                        on:click=handle_toggle_bookmark.clone()
                                        aria-label=bm_label
                                        title=bm_title
                                    >
                                        <IconBookmark size=13 filled=bookmarked />
                                    </button>
                                }
                            })}
                            // Copy
                            {has_plain_text.then(|| {
                                view! {
                                    <button
                                        class="message-action-btn"
                                        on:click=handle_copy.clone()
                                        aria-label="Copy message"
                                        title="Copy message"
                                    >
                                        {move || if copied.get() {
                                            view! {
                                                <IconCheck size=13 />
                                            }.into_any()
                                        } else {
                                            view! {
                                                <IconCopy size=13 />
                                            }.into_any()
                                        }}
                                    </button>
                                }
                            })}
                            // Retry (user only)
                            {has_retry.then(|| view! {
                                <button
                                    class="message-action-btn"
                                    on:click=handle_retry.clone()
                                    aria-label="Retry message"
                                    title="Retry message"
                                >
                                    <IconRotateCcw size=13 />
                                </button>
                            })}
                            // Model label in action bar (React: assistant only)
                            {model_for_bar.map(|m| {
                                let label = model_label(&m);
                                view! {
                                    <span class="message-actions-model">{label}</span>
                                }
                            })}
                        </div>
                    }
                })}
            </div>
        </div>
    }.into_any()
}

/// Render markdown text as a message-body div with proper HTML rendering.
/// Fenced code blocks are extracted and rendered via CodeBlock component.
fn render_markdown_body(text: &str) -> leptos::prelude::AnyView {
    let segments = parse_markdown_segments(text);
    view! {
        <div class="message-body">
            {segments.into_iter().map(|seg| {
                match seg {
                    ContentSegment::Html(html) => {
                        view! {
                            <div inner_html=html></div>
                        }.into_any()
                    }
                    ContentSegment::FencedCode { language, code } => {
                        view! {
                            <CodeBlock language=language code=code />
                        }.into_any()
                    }
                }
            }).collect_view()}
        </div>
    }.into_any()
}

/// Render a user message that has memory guidance — accordion with user text as title,
/// memory list as collapsible content.
fn render_user_memory_accordion(user_text: &str, memory_text: &str) -> leptos::prelude::AnyView {
    let (open, set_open) = signal(false);
    let user_segments = parse_markdown_segments(user_text);
    let mem_segments = parse_markdown_segments(memory_text);

    view! {
        <div class="message-body">
            // User message always visible
            {user_segments.into_iter().map(|seg| {
                match seg {
                    ContentSegment::Html(html) => view! { <div inner_html=html></div> }.into_any(),
                    ContentSegment::FencedCode { language, code } => {
                        view! { <CodeBlock language=language code=code /> }.into_any()
                    }
                }
            }).collect_view()}
            // Memory accordion toggle
            <button
                class="user-memory-toggle"
                on:click=move |_| set_open.update(|v| *v = !*v)
            >
                <svg
                    class="user-memory-chevron"
                    class:user-memory-chevron-open=move || open.get()
                    width="12" height="12" viewBox="0 0 24 24"
                    fill="none" stroke="currentColor" stroke-width="2"
                    stroke-linecap="round" stroke-linejoin="round"
                >
                    <polyline points="9 18 15 12 9 6" />
                </svg>
                <span class="user-memory-label">"Applied memories"</span>
            </button>
            <div
                class="user-memory-content"
                style:display=move || if open.get() { "" } else { "none" }
            >
                {mem_segments.into_iter().map(|seg| {
                    match seg {
                        ContentSegment::Html(html) => view! { <div inner_html=html></div> }.into_any(),
                        ContentSegment::FencedCode { language, code } => {
                            view! { <CodeBlock language=language code=code /> }.into_any()
                        }
                    }
                }).collect_view()}
            </div>
        </div>
    }.into_any()
}

/// Render parts in order, grouping consecutive text parts together
/// and rendering tool calls inline between text blocks.
fn render_interleaved(
    all_parts: &[MessagePart],
    child_sessions: &[SessionInfo],
    subagent_messages: Option<ReadSignal<SubagentMessagesMap>>,
    on_open_session: Option<Callback<String>>,
) -> leptos::prelude::AnyView {
    let mut elements: Vec<leptos::prelude::AnyView> = Vec::new();
    let mut current_text_chunks: Vec<String> = Vec::new();
    let mut task_tool_index: usize = 0;

    // ── A2UI delta merge ────────────────────────────────────────────
    // For A2UI tool calls sharing a render_id, collect all inputs in
    // order and pre-merge blocks. Only the LAST part renders; its input
    // is replaced with the merged result so A2uiBlocks is stateless.
    let (a2ui_last_index, a2ui_merged) = {
        use std::collections::HashMap;
        // Pass 1: group part indices by render_id
        let mut rid_parts: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, part) in all_parts.iter().enumerate() {
            let tn = part.tool.as_deref()
                .or(part.tool_name.as_deref())
                .unwrap_or("");
            if tn != "ui_render" && tn != "ui_ui_render" {
                continue;
            }
            let rid = part.state.as_ref()
                .and_then(|s| s.input.as_ref())
                .and_then(|inp| inp.get("render_id"))
                .and_then(|v| v.as_str());
            if let Some(rid) = rid {
                rid_parts.entry(rid.to_string()).or_default().push(i);
            }
        }
        // Pass 2: build last-index map + merged inputs
        let mut last_map = HashMap::<String, usize>::new();
        let mut merged_map = HashMap::<usize, serde_json::Value>::new();
        for (rid, indices) in &rid_parts {
            let last_idx = *indices.last().unwrap();
            last_map.insert(rid.clone(), last_idx);
            if indices.len() == 1 {
                continue; // no merge needed, single call
            }
            // Merge blocks from all parts in order
            let mut merged_blocks: Vec<serde_json::Value> = Vec::new();
            for &idx in indices {
                let inp = all_parts[idx].state.as_ref()
                    .and_then(|s| s.input.as_ref());
                let Some(inp) = inp else { continue };
                let op = inp.get("operation")
                    .and_then(|v| v.as_str())
                    .unwrap_or("replace");
                let blocks = inp.get("blocks")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                match op {
                    "append" => merged_blocks.extend(blocks),
                    "update" => {
                        for block in &blocks {
                            if let Some(bi) = block.get("index").and_then(|v| v.as_u64()) {
                                let bi = bi as usize;
                                if bi < merged_blocks.len() {
                                    if let Some(ud) = block.get("data").and_then(|d| d.as_object()) {
                                        if let Some(ed) = merged_blocks[bi].get_mut("data")
                                            .and_then(|d| d.as_object_mut())
                                        {
                                            for (k, v) in ud {
                                                ed.insert(k.clone(), v.clone());
                                            }
                                        }
                                    }
                                    if let Some(t) = block.get("type") {
                                        if let Some(obj) = merged_blocks[bi].as_object_mut() {
                                            obj.insert("type".to_string(), t.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => merged_blocks = blocks, // replace
                }
            }
            // Build merged input: copy last part's input, override blocks
            if let Some(base) = all_parts[last_idx].state.as_ref()
                .and_then(|s| s.input.clone())
            {
                let mut merged_input = base;
                if let Some(obj) = merged_input.as_object_mut() {
                    obj.insert("blocks".to_string(),
                        serde_json::Value::Array(merged_blocks));
                    // Remove operation so A2uiBlocks treats as plain render
                    obj.remove("operation");
                }
                merged_map.insert(last_idx, merged_input);
            }
        }
        (last_map, merged_map)
    };

    let flush_text = |chunks: &mut Vec<String>, elems: &mut Vec<leptos::prelude::AnyView>| {
        if !chunks.is_empty() {
            let text = chunks.join("\n");
            let segments = parse_markdown_segments(&text);
            elems.push(view! {
                <div class="message-body">
                    {segments.into_iter().map(|seg| {
                        match seg {
                            ContentSegment::Html(html) => {
                                view! {
                                    <div inner_html=html></div>
                                }.into_any()
                            }
                            ContentSegment::FencedCode { language, code } => {
                                view! { <CodeBlock language=language code=code /> }.into_any()
                            }
                        }
                    }).collect_view()}
                </div>
            }.into_any());
            chunks.clear();
        }
    };

    for (part_idx, part) in all_parts.iter().enumerate() {
        if part.part_type == "text" {
            if let Some(ref text) = part.text {
                current_text_chunks.push(text.clone());
            }
        } else if part.part_type == "tool" || part.part_type == "tool-call" || part.part_type == "tool_call" {
            flush_text(&mut current_text_chunks, &mut elements);

            let tool_name = part.tool.clone().or_else(|| part.tool_name.clone()).unwrap_or_default();

            // Skip stale A2UI parts (earlier render_id superseded by a later one)
            if tool_name == "ui_render" || tool_name == "ui_ui_render" {
                let rid = part.state.as_ref()
                    .and_then(|s| s.input.as_ref())
                    .and_then(|inp| inp.get("render_id"))
                    .and_then(|v| v.as_str());
                if let Some(rid) = rid {
                    if a2ui_last_index.get(rid).copied() != Some(part_idx) {
                        continue; // stale — a later call with same render_id exists
                    }
                }
            }

            let is_task = tool_name == "task";
            let matched = if is_task {
                child_sessions.get(task_tool_index).map(|s| ChildSessionRef {
                    id: s.id.clone(),
                    title: s.title.clone(),
                })
            } else {
                None
            };
            if is_task {
                task_tool_index += 1;
            }

            // For A2UI with merged delta, swap in the pre-merged input
            let part_clone = if let Some(merged_input) = a2ui_merged.get(&part_idx) {
                let mut p = part.clone();
                if let Some(ref mut state) = p.state {
                    state.input = Some(merged_input.clone());
                }
                p
            } else {
                part.clone()
            };

            let sub_msgs = subagent_messages.clone();
            let on_open = on_open_session.clone();

            elements.push(view! {
                <ToolCallView
                    part=part_clone
                    child_session=matched
                    subagent_messages=sub_msgs
                    on_open_session=on_open
                />
            }.into_any());
        }
    }

    flush_text(&mut current_text_chunks, &mut elements);

    view! {
        <div class="message-interleaved">
            {elements}
        </div>
    }.into_any()
}
