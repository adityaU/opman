//! QuestionDock — displays pending questions with text/select/confirm inputs.
//! Matches React `QuestionDock.tsx`.

use crate::components::icons::*;
use crate::types::core::QuestionRequest;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

/// QuestionDock component.
#[component]
pub fn QuestionDock(
    questions: Memo<Vec<QuestionRequest>>,
    active_session_id: Memo<Option<String>>,
    on_reply: Callback<(String, Vec<Vec<String>>)>,
    on_dismiss: Callback<String>,
    on_go_to_session: Callback<String>,
) -> impl IntoView {
    let (active_tab, set_active_tab) = signal(0usize);

    // Clamp active_tab when questions list changes
    Effect::new(move |_| {
        let len = questions.get().len();
        if active_tab.get_untracked() >= len && len > 0 {
            set_active_tab.set(len - 1);
        } else if len == 0 {
            set_active_tab.set(0);
        }
    });

    let show = Memo::new(move |_| !questions.get().is_empty());

    view! {
        {move || {
            if !show.get() {
                return None;
            }

            let qs = questions.get();
            let show_tabs = qs.len() > 1;
            let idx = active_tab.get().min(qs.len().saturating_sub(1));
            let active_q = qs.get(idx).cloned();

            Some(view! {
                <div
                    class="question-dock mx-4 mb-2 rounded-lg border border-info/30 bg-bg-panel/95 backdrop-blur-sm shadow-lg overflow-hidden"
                    role="region"
                    aria-label="Questions"
                >
                    // Tab strip for multiple questions
                    {if show_tabs {
                        let tabs = qs.iter().enumerate().map(|(i, q)| {
                            let title = q.title.clone();
                            let is_cross = {
                                let asid = active_session_id.get_untracked();
                                asid.as_ref().map_or(false, |s| s != &q.session_id)
                            };
                            let label = if title.is_empty() { format!("Question {}", i + 1) } else { title };
                            view! {
                                <button
                                    class=move || {
                                        let base = "flex items-center gap-1 px-2.5 py-1 text-xs rounded-t transition-colors";
                                        if active_tab.get() == i {
                                            format!("{} bg-info/20 text-info border-b-2 border-info", base)
                                        } else {
                                            format!("{} text-text-muted hover:text-text hover:bg-bg-hover", base)
                                        }
                                    }
                                    on:click=move |_| set_active_tab.set(i)
                                >
                                    <span class="text-info">"?"</span>
                                    <span class="truncate max-w-[100px]">{label.clone()}</span>
                                    {if is_cross {
                                        Some(view! {
                                            <span class="text-[10px] px-1 rounded bg-primary/20 text-primary">"sub"</span>
                                        })
                                    } else {
                                        None
                                    }}
                                </button>
                            }
                        }).collect::<Vec<_>>();
                        Some(view! {
                            <div class="flex items-center gap-0.5 px-2 pt-1 border-b border-border-subtle overflow-x-auto">
                                {tabs}
                            </div>
                        })
                    } else {
                        None
                    }}

                    // Active question card
                    {if let Some(q) = active_q {
                        let is_cross = {
                            let asid = active_session_id.get();
                            asid.as_ref().map_or(false, |s| s != &q.session_id)
                        };
                        Some(view! {
                            <QuestionCard
                                question=q
                                is_cross_session=is_cross
                                on_reply=on_reply
                                on_dismiss=on_dismiss
                                on_go_to_session=Some(on_go_to_session)
                            />
                        })
                    } else {
                        None
                    }}
                </div>
            })
        }}
    }
}

/// Single question card with inputs for each sub-question.
#[component]
fn QuestionCard(
    question: QuestionRequest,
    is_cross_session: bool,
    on_reply: Callback<(String, Vec<Vec<String>>)>,
    on_dismiss: Callback<String>,
    on_go_to_session: Option<Callback<String>>,
) -> impl IntoView {
    let q_id = question.id.clone();
    let q_id_submit = q_id.clone();
    let q_id_dismiss = q_id.clone();
    let q_id_key = q_id.clone();

    let title = question.title.clone();
    let session_id_short = question.session_id.chars().take(8).collect::<String>();
    let num_questions = question.questions.len();

    // Auto-focus the first interactive element when the card mounts (matches React QuestionDock).
    let card_ref = NodeRef::<leptos::html::Div>::new();
    Effect::new(move |_| {
        if let Some(el) = card_ref.get() {
            let el: web_sys::HtmlElement = el.into();
            gloo_timers::callback::Timeout::new(50, move || {
                // Focus first input/button inside the card body, matching React autoFocus behavior
                let target = el
                    .query_selector("input, button.flex.flex-col, button")
                    .ok()
                    .flatten();
                if let Some(t) = target {
                    if let Ok(html) = t.dyn_into::<web_sys::HtmlElement>() {
                        let _ = html.focus();
                        return;
                    }
                }
                let _ = el.focus();
            })
            .forget();
        }
    });

    // Per-question answer signals: Vec<RwSignal<Vec<String>>>
    let answer_signals: Vec<RwSignal<Vec<String>>> = (0..num_questions)
        .map(|_| RwSignal::new(Vec::<String>::new()))
        .collect();

    // Per-question custom text signals (for select with custom=true)
    let custom_text_signals: Vec<RwSignal<String>> = (0..num_questions)
        .map(|_| RwSignal::new(String::new()))
        .collect();

    let answer_signals_submit = answer_signals.clone();
    let custom_text_submit = custom_text_signals.clone();
    let questions_for_submit = question.questions.clone();

    let submit = move || {
        use leptos::callback::Callable;
        let final_answers: Vec<Vec<String>> = questions_for_submit
            .iter()
            .enumerate()
            .map(|(_idx, _q)| {
                let selected = answer_signals_submit[_idx].get_untracked();
                let custom = custom_text_submit[_idx].get_untracked();
                let custom = custom.trim().to_string();
                if !custom.is_empty() && selected.is_empty() {
                    vec![custom]
                } else if !custom.is_empty() && !selected.is_empty() {
                    let mut combined = selected;
                    combined.push(custom);
                    combined
                } else {
                    selected
                }
            })
            .collect();
        on_reply.run((q_id_submit.clone(), final_answers));
    };

    let dismiss = {
        let q_id_d = q_id_dismiss.clone();
        move || {
            use leptos::callback::Callable;
            on_dismiss.run(q_id_d.clone());
        }
    };

    // Check if all questions have at least one answer
    let answer_sigs_check = answer_signals.clone();
    let custom_sigs_check = custom_text_signals.clone();
    let questions_for_check = question.questions.clone();
    let has_answer = Memo::new(move |_| {
        questions_for_check.iter().enumerate().all(|(idx, q)| {
            let selected = answer_sigs_check[idx].get();
            let custom = custom_sigs_check[idx].get();
            let custom = custom.trim().to_string();
            if q.question_type == "text" {
                !selected.first().map_or(true, |s| s.trim().is_empty())
            } else {
                !selected.is_empty() || !custom.is_empty()
            }
        })
    });

    let submit_clone = submit.clone();
    let dismiss_clone = dismiss.clone();

    // Build question items
    let items = question.questions.iter().enumerate().map(|(idx, q_item)| {
        let text = q_item.text.clone();
        let q_type = q_item.question_type.clone();
        let options = q_item.options.clone().unwrap_or_default();
        let option_descs = q_item.option_descriptions.clone().unwrap_or_default();
        let is_multiple = q_item.multiple.unwrap_or(false);
        let has_custom = q_item.custom.unwrap_or(true); // default true per React
        let answer_sig = answer_signals[idx];
        let custom_sig = custom_text_signals[idx];

        let content = if q_type == "select" && !options.is_empty() {
            let option_buttons = options.iter().enumerate().map(|(opt_idx, opt)| {
                let opt_val = opt.clone();
                let opt_val_click = opt_val.clone();
                let opt_label = opt.clone();
                let desc = option_descs.get(opt_idx).cloned();
                let is_multi = is_multiple;
                view! {
                    <button
                        class=move || {
                            let selected = answer_sig.get();
                            let is_sel = selected.contains(&opt_val);
                            let base = "flex flex-col items-start px-3 py-1.5 rounded text-xs transition-colors border";
                            if is_sel {
                                format!("{} border-info bg-info/20 text-info", base)
                            } else {
                                format!("{} border-border-subtle bg-bg-hover text-text hover:border-info/50", base)
                            }
                        }
                        on:click={
                            let ov = opt_val_click;
                            move |_| {
                                let mut current = answer_sig.get_untracked();
                                if is_multi {
                                    if current.contains(&ov) {
                                        current.retain(|a| a != &ov);
                                    } else {
                                        current.push(ov.clone());
                                    }
                                    answer_sig.set(current);
                                } else {
                                    custom_sig.set(String::new());
                                    answer_sig.set(vec![ov.clone()]);
                                }
                            }
                        }
                    >
                        <span class="font-medium">{opt_label}</span>
                        {desc.map(|d| view! {
                            <span class="text-text-muted text-[10px]">{d}</span>
                        })}
                    </button>
                }
            }).collect::<Vec<_>>();

            let custom_input = if has_custom {
                Some(view! {
                    <input
                        type="text"
                        class="w-full mt-1.5 px-2.5 py-1.5 rounded bg-bg-hover border border-border-subtle text-xs text-text placeholder:text-text-muted focus:outline-none focus:border-info/50"
                        placeholder="Type your own answer..."
                        prop:value=move || custom_sig.get()
                        on:input=move |e| {
                            let v = event_target_value(&e);
                            custom_sig.set(v.clone());
                            if !v.is_empty() {
                                answer_sig.set(Vec::new());
                            }
                        }
                    />
                })
            } else {
                None
            };

            view! {
                <div>
                    <div class="flex flex-wrap gap-1.5">{option_buttons}</div>
                    {custom_input}
                </div>
            }.into_any()
        } else if q_type == "confirm" {
            view! {
                <div class="flex gap-2">
                    <button
                        class=move || {
                            let sel = answer_sig.get();
                            let is_yes = sel.first().map_or(false, |s| s == "yes");
                            let base = "px-4 py-1.5 rounded text-xs font-medium transition-colors border";
                            if is_yes {
                                format!("{} border-success bg-success/20 text-success", base)
                            } else {
                                format!("{} border-border-subtle bg-bg-hover text-text hover:border-success/50", base)
                            }
                        }
                        on:click=move |_| answer_sig.set(vec!["yes".to_string()])
                    >
                        "Yes"
                    </button>
                    <button
                        class=move || {
                            let sel = answer_sig.get();
                            let is_no = sel.first().map_or(false, |s| s == "no");
                            let base = "px-4 py-1.5 rounded text-xs font-medium transition-colors border";
                            if is_no {
                                format!("{} border-error bg-error/20 text-error", base)
                            } else {
                                format!("{} border-border-subtle bg-bg-hover text-text hover:border-error/50", base)
                            }
                        }
                        on:click=move |_| answer_sig.set(vec!["no".to_string()])
                    >
                        "No"
                    </button>
                </div>
            }.into_any()
        } else {
            // text input (default)
            view! {
                <input
                    type="text"
                    class="w-full px-2.5 py-1.5 rounded bg-bg-hover border border-border-subtle text-xs text-text placeholder:text-text-muted focus:outline-none focus:border-info/50"
                    placeholder="Type your answer..."
                    prop:value=move || answer_sig.get().first().cloned().unwrap_or_default()
                    on:input=move |e| {
                        let v = event_target_value(&e);
                        answer_sig.set(vec![v]);
                    }
                />
            }.into_any()
        };

        view! {
            <div class="space-y-1">
                <label class="text-xs font-medium text-text">{text}</label>
                {content}
            </div>
        }
    }).collect::<Vec<_>>();

    view! {
        <div
            node_ref=card_ref
            class="question-card"
            tabindex="0"
            on:keydown=move |e| {
                let key = e.key();
                if key == "Escape" {
                    e.prevent_default();
                    (dismiss_clone.clone())();
                } else if key == "Enter" && has_answer.get_untracked() {
                    // Only submit on Enter if not in a text input or Ctrl/Meta held
                    let target: web_sys::HtmlElement = e.target().unwrap().unchecked_into();
                    let tag = target.tag_name();
                    let is_text_input = tag == "INPUT";
                    if !is_text_input || e.meta_key() || e.ctrl_key() {
                        e.prevent_default();
                        (submit_clone.clone())();
                    }
                }
            }
        >
            // Header
            <div class="flex items-center gap-2 px-3 py-2 border-b border-border-subtle">
                <span class="text-info font-bold">"?"</span>
                <span class="text-sm font-medium text-text">
                    {if title.is_empty() { "Question".to_string() } else { title }}
                </span>
                {if is_cross_session {
                    Some(view! {
                        <span class="question-badge-subagent text-[10px] px-1.5 py-0.5 rounded bg-primary/20 text-primary font-medium">"subagent"</span>
                    })
                } else {
                    None
                }}
                {if !question.session_id.is_empty() {
                    if let Some(go) = on_go_to_session {
                        let sid = question.session_id.clone();
                        let sid_short = session_id_short.clone();
                        Some(view! {
                            <button
                                class="dock-session-link flex items-center gap-0.5 text-[10px] text-text-muted hover:text-primary transition-colors"
                                on:click=move |e: web_sys::MouseEvent| {
                                    e.stop_propagation();
                                    go.run(sid.clone());
                                }
                                title=format!("Go to session {}", sid_short)
                                aria-label="Go to session"
                            >
                                <IconExternalLink size=11 />
                                <span>{sid_short.clone()}</span>
                            </button>
                        }.into_any())
                    } else {
                        Some(view! {
                            <span class="text-[10px] text-text-muted ml-1">{session_id_short.clone()}</span>
                        }.into_any())
                    }
                } else {
                    None
                }}
                <span class="flex-1" />
                <span class="text-[10px] text-text-muted hidden sm:inline">
                    "Enter = submit \u{00b7} Esc = dismiss"
                </span>
                <button
                    class="text-text-muted hover:text-error text-sm transition-colors"
                    on:click={
                        let q_id_d2 = q_id_dismiss.clone();
                        move |_| {
                            use leptos::callback::Callable;
                            on_dismiss.run(q_id_d2.clone());
                        }
                    }
                    title="Dismiss (Esc)"
                >
                    "x"
                </button>
            </div>

            // Body — question items
            <div class="px-3 py-2 space-y-3 max-h-64 overflow-y-auto">
                {items}
            </div>

            // Actions
            <div class="flex items-center gap-2 px-3 py-2 border-t border-border-subtle">
                <button
                    class=move || {
                        let base = "flex items-center gap-1 px-3 py-1.5 rounded text-xs font-medium transition-colors";
                        if has_answer.get() {
                            format!("{} bg-info/20 text-info hover:bg-info/30", base)
                        } else {
                            format!("{} bg-bg-hover text-text-muted cursor-not-allowed opacity-50", base)
                        }
                    }
                    prop:disabled=move || !has_answer.get()
                    on:click={
                        let q_id_s = q_id.clone();
                        let answer_sigs = answer_signals.clone();
                        let custom_sigs = custom_text_signals.clone();
                        let qs = question.questions.clone();
                        move |_| {
                            use leptos::callback::Callable;
                            let final_answers: Vec<Vec<String>> = qs.iter().enumerate().map(|(i, _)| {
                                let selected = answer_sigs[i].get_untracked();
                                let custom = custom_sigs[i].get_untracked();
                                let custom = custom.trim().to_string();
                                if !custom.is_empty() && selected.is_empty() {
                                    vec![custom]
                                } else if !custom.is_empty() && !selected.is_empty() {
                                    let mut combined = selected;
                                    combined.push(custom);
                                    combined
                                } else {
                                    selected
                                }
                            }).collect();
                            on_reply.run((q_id_s.clone(), final_answers));
                        }
                    }
                >
                    <span>">"</span>
                    "Submit"
                </button>
            </div>
        </div>
    }
}
