//! QuestionDock — displays pending questions with text/select/confirm inputs.
//! Matches React `QuestionDock.tsx` using CSS classes from `permissions-1.css`.

use crate::components::icons::*;
use crate::types::core::QuestionRequest;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

/// QuestionDock component — outer wrapper with tabs for multiple questions.
#[component]
pub fn QuestionDock(
    questions: Memo<Vec<QuestionRequest>>,
    active_session_id: Memo<Option<String>>,
    on_reply: Callback<(String, Vec<Vec<String>>)>,
    on_dismiss: Callback<String>,
    on_go_to_session: Callback<String>,
) -> impl IntoView {
    let (active_tab, set_active_tab) = signal(0usize);
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
            if !show.get() { return None; }
            let qs = questions.get();
            let idx = active_tab.get().min(qs.len().saturating_sub(1));
            let active_q = qs.get(idx).cloned();
            let tabs_view = (qs.len() > 1).then(|| {
                let tabs = qs.iter().enumerate().map(|(i, q)| {
                    let is_cross = active_session_id.get_untracked()
                        .as_ref().map_or(false, |s| s != &q.session_id);
                    let label = if q.title.is_empty() { format!("Question {}", i + 1) } else { q.title.clone() };
                    view! {
                        <button
                            class=move || if active_tab.get() == i {
                                "dock-tab dock-tab--question dock-tab--active"
                            } else { "dock-tab dock-tab--question" }
                            on:click=move |_| set_active_tab.set(i)
                            aria-selected=move || active_tab.get() == i
                            role="tab"
                        >
                            <IconHelpCircle size=12 />
                            <span class="dock-tab-label">{label.clone()}</span>
                            {is_cross.then(|| view! { <span class="dock-tab-badge">"sub"</span> })}
                        </button>
                    }
                }).collect::<Vec<_>>();
                view! { <div class="dock-tabs dock-tabs--question">{tabs}</div> }
            });
            Some(view! {
                <div class="question-dock" role="region" aria-label="Questions">
                    {tabs_view}
                    {active_q.map(|q| {
                        let is_cross = active_session_id.get()
                            .as_ref().map_or(false, |s| s != &q.session_id);
                        view! {
                            <QuestionCard question=q is_cross_session=is_cross
                                on_reply=on_reply on_dismiss=on_dismiss
                                on_go_to_session=Some(on_go_to_session) />
                        }
                    })}
                </div>
            })
        }}
    }
}

fn opt_class(selected: bool) -> &'static str {
    if selected {
        "question-option selected"
    } else {
        "question-option"
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
    let title = question.title.clone();
    let sid_short = question.session_id.chars().take(8).collect::<String>();
    let num_q = question.questions.len();

    let card_ref = NodeRef::<leptos::html::Div>::new();
    Effect::new(move |_| {
        let Some(el) = card_ref.get() else { return };
        let el: web_sys::HtmlElement = el.into();
        gloo_timers::callback::Timeout::new(50, move || {
            if let Some(Ok(html)) = el
                .query_selector("input, button")
                .ok()
                .flatten()
                .map(|t| t.dyn_into::<web_sys::HtmlElement>())
            {
                let _ = html.focus();
            } else {
                let _ = el.focus();
            }
        })
        .forget();
    });

    let ans: Vec<RwSignal<Vec<String>>> = (0..num_q).map(|_| RwSignal::new(Vec::new())).collect();
    let cust: Vec<RwSignal<String>> = (0..num_q).map(|_| RwSignal::new(String::new())).collect();

    let build_answers = {
        let (a, c, qs) = (ans.clone(), cust.clone(), question.questions.clone());
        move || -> Vec<Vec<String>> {
            qs.iter()
                .enumerate()
                .map(|(i, _)| {
                    let sel = a[i].get_untracked();
                    let ct = c[i].get_untracked().trim().to_string();
                    if ct.is_empty() {
                        return sel;
                    }
                    if sel.is_empty() {
                        return vec![ct];
                    }
                    let mut r = sel;
                    r.push(ct);
                    r
                })
                .collect()
        }
    };
    let has_answer = {
        let (a, c, qs) = (ans.clone(), cust.clone(), question.questions.clone());
        Memo::new(move |_| {
            qs.iter().enumerate().all(|(i, q)| {
                let sel = a[i].get();
                let ct = c[i].get();
                if q.question_type == "text" {
                    return !sel.first().map_or(true, |s| s.trim().is_empty());
                }
                !sel.is_empty() || !ct.trim().is_empty()
            })
        })
    };

    let (qid_s, qid_d, qid_d2) = (q_id.clone(), q_id.clone(), q_id.clone());
    let (ba_s, ba_k) = (build_answers.clone(), build_answers.clone());

    let items = question.questions.iter().enumerate().map(|(idx, qi)| {
        let text = qi.text.clone();
        let q_type = qi.question_type.clone();
        let opts = qi.options.clone().unwrap_or_default();
        let descs = qi.option_descriptions.clone().unwrap_or_default();
        let multi = qi.multiple.unwrap_or(false);
        let custom_ok = qi.custom.unwrap_or(true);
        let (asig, csig) = (ans[idx], cust[idx]);

        let content = if q_type == "select" && !opts.is_empty() {
            let btns = opts.iter().enumerate().map(|(oi, opt)| {
                let (v, v2, vc, label) = (opt.clone(), opt.clone(), opt.clone(), opt.clone());
                let desc = descs.get(oi).cloned();
                view! {
                    <button
                        class=move || opt_class(asig.get().contains(&v))
                        role="option"
                        aria-selected=move || asig.get().contains(&v2)
                        on:click={let ov = vc; move |_| {
                            let mut cur = asig.get_untracked();
                            if multi {
                                if cur.contains(&ov) { cur.retain(|a| a != &ov); }
                                else { cur.push(ov.clone()); }
                                asig.set(cur);
                            } else {
                                csig.set(String::new());
                                asig.set(vec![ov.clone()]);
                            }
                        }}
                    >
                        <span class="question-option-label">{label}</span>
                        {desc.map(|d| view! { <span class="question-option-desc">{d}</span> })}
                    </button>
                }
            }).collect::<Vec<_>>();
            let cust_inp = custom_ok.then(|| view! {
                <input type="text" class="question-text-input question-custom-input"
                    placeholder="Type your own answer..."
                    prop:value=move || csig.get()
                    on:input=move |e| {
                        let v = event_target_value(&e);
                        csig.set(v.clone());
                        if !v.is_empty() { asig.set(Vec::new()); }
                    }
                    aria-label=format!("Custom answer for: {}", text)
                />
            });
            view! {
                <div class="question-options" role="listbox" aria-label=text.clone()>{btns}</div>
                {cust_inp}
            }.into_any()
        } else if q_type == "confirm" {
            view! {
                <div class="question-options" role="group" aria-label=text.clone()>
                    <button class=move || opt_class(asig.get().first().map_or(false, |s| s == "yes"))
                        on:click=move |_| asig.set(vec!["yes".into()])>"Yes"</button>
                    <button class=move || opt_class(asig.get().first().map_or(false, |s| s == "no"))
                        on:click=move |_| asig.set(vec!["no".into()])>"No"</button>
                </div>
            }.into_any()
        } else {
            view! {
                <input type="text" class="question-text-input" placeholder="Type your answer..."
                    prop:value=move || asig.get().first().cloned().unwrap_or_default()
                    on:input=move |e| asig.set(vec![event_target_value(&e)])
                    aria-label=text.clone() />
            }.into_any()
        };
        view! { <div class="question-item"><label class="question-label">{text}</label>{content}</div> }
    }).collect::<Vec<_>>();

    view! {
        <div node_ref=card_ref class="question-card" tabindex="0"
            on:keydown=move |e| {
                if e.key() == "Escape" {
                    e.prevent_default();
                    on_dismiss.run(qid_d.clone());
                } else if e.key() == "Enter" && has_answer.get_untracked() {
                    let tag = e.target().unwrap().unchecked_into::<web_sys::HtmlElement>().tag_name();
                    if tag != "INPUT" || e.meta_key() || e.ctrl_key() {
                        e.prevent_default();
                        on_reply.run((qid_d.clone(), (ba_k.clone())()));
                    }
                }
            }
        >
            <div class="question-header">
                <IconHelpCircle size=16 class="question-icon" />
                <span class="question-title">
                    {if title.is_empty() { "Question".to_string() } else { title }}
                </span>
                {is_cross_session.then(|| view! { <span class="question-badge-subagent">"subagent"</span> })}
                {(!question.session_id.is_empty()).then(|| {
                    if let Some(go) = on_go_to_session {
                        let (sid, short) = (question.session_id.clone(), sid_short.clone());
                        view! {
                            <button class="dock-session-link"
                                on:click=move |e: web_sys::MouseEvent| { e.stop_propagation(); go.run(sid.clone()); }
                                title=format!("Go to session {}", short) aria-label="Go to session">
                                <IconExternalLink size=11 /><span>{short.clone()}</span>
                            </button>
                        }.into_any()
                    } else {
                        view! { <span class="dock-session-link">{sid_short.clone()}</span> }.into_any()
                    }
                })}
                <span class="question-hint">"Enter = submit \u{00b7} Esc = dismiss"</span>
                <button class="question-dismiss-btn"
                    on:click=move |_| on_dismiss.run(qid_d2.clone())
                    aria-label="Dismiss question" title="Dismiss (Esc)">
                    <IconX size=14 />
                </button>
            </div>
            <div class="question-body">{items}</div>
            <div class="question-actions">
                <button class="question-submit-btn" prop:disabled=move || !has_answer.get()
                    on:click=move |_| on_reply.run((qid_s.clone(), (ba_s.clone())()))
                    aria-label="Submit answers">
                    <IconSend size=14 />"Submit"
                </button>
            </div>
        </div>
    }
}
