//! Process Health Modal — desktop modal / mobile bottom drawer.
//!
//! Shows mitigation toggles, live snapshot metrics, and recent audit log.
//! Rendered via ModalLayer (ModalName::ProcessHealth), same pattern as CommandPalette.

use leptos::prelude::*;

use crate::api::health::{
    self, AuditEntry, HealthStatusResponse, MitigationInfo,
};
use crate::components::icons::*;

/// ProcessHealthDrawer — modal on desktop, bottom sheet on mobile.
#[component]
pub fn ProcessHealthDrawer(
    on_close: Callback<()>,
) -> impl IntoView {
    let (status, set_status) = signal::<Option<HealthStatusResponse>>(None);
    let (audit, set_audit) = signal::<Vec<AuditEntry>>(Vec::new());
    let (loading, set_loading) = signal(false);
    let (tab, set_tab) = signal::<&'static str>("overview");

    // Fetch data on mount
    let fetch_all = move || {
        set_loading.set(true);
        leptos::task::spawn_local(async move {
            let (s, a) = futures::join!(
                health::fetch_health_status(),
                health::fetch_health_audit(100),
            );
            if let Ok(s) = s { set_status.set(Some(s)); }
            if let Ok(a) = a { set_audit.set(a.entries); }
            set_loading.set(false);
        });
    };

    // Auto-fetch on mount
    fetch_all();

    // Toggle handler
    let on_toggle = Callback::new(move |(mitigation_id, enabled): (String, bool)| {
        leptos::task::spawn_local(async move {
            if let Ok(s) = health::toggle_mitigation(&mitigation_id, enabled).await {
                set_status.set(Some(s));
            }
        });
    });

    // Keyboard: Escape closes
    let on_keydown = move |e: web_sys::KeyboardEvent| {
        if e.key() == "Escape" {
            e.prevent_default();
            on_close.run(());
        }
    };

    view! {
        <div class="modal-backdrop" on:click=move |_| on_close.run(())>
            <div
                class="health-modal"
                role="dialog"
                aria-modal="true"
                on:click=|e: web_sys::MouseEvent| e.stop_propagation()
                on:keydown=on_keydown
            >
                // Header
                <div class="health-drawer-header">
                    <div class="health-drawer-title">
                        <IconCpu size=14 />
                        <span>"Process Health"</span>
                    </div>
                    <div class="health-drawer-tabs">
                        <button
                            class=move || if tab.get() == "overview" { "health-tab active" } else { "health-tab" }
                            on:click=move |_| set_tab.set("overview")
                        >"Overview"</button>
                        <button
                            class=move || if tab.get() == "audit" { "health-tab active" } else { "health-tab" }
                            on:click=move |_| set_tab.set("audit")
                        >"Audit Log"</button>
                    </div>
                    <div class="health-drawer-actions">
                        <button
                            class="health-refresh-btn"
                            on:click=move |_| fetch_all()
                            title="Refresh"
                            disabled=move || loading.get()
                        >
                            <IconRefreshCw size=12 />
                        </button>
                        <button
                            class="health-close-btn"
                            on:click=move |_| on_close.run(())
                            aria-label="Close"
                        >
                            <IconX size=14 />
                        </button>
                    </div>
                </div>

                // Body
                <div class="health-drawer-body">
                    {move || {
                        if loading.get() && status.get().is_none() {
                            return view! {
                                <div class="health-loading">"Loading..."</div>
                            }.into_any();
                        }
                        match tab.get() {
                            "audit" => view! {
                                <AuditLogTab entries=audit />
                            }.into_any(),
                            _ => view! {
                                <OverviewTab status=status on_toggle=on_toggle />
                            }.into_any(),
                        }
                    }}
                </div>
            </div>
        </div>
    }
}

/// Overview tab: toggles + metrics snapshot.
#[component]
fn OverviewTab(
    status: ReadSignal<Option<HealthStatusResponse>>,
    on_toggle: Callback<(String, bool)>,
) -> impl IntoView {
    view! {
        <div class="health-overview">
            {move || {
                let s = status.get();
                match s {
                    None => view! { <div class="health-empty">"No data"</div> }.into_any(),
                    Some(s) => {
                        let mitigations = s.mitigations.clone();
                        let snap = s.snapshot.clone();
                        view! {
                            <div class="health-grid">
                                <div class="health-section">
                                    <div class="health-section-title">"Mitigations"</div>
                                    <div class="health-toggles">
                                        {mitigations.into_iter().map(|m| {
                                            let id = m.id.clone();
                                            view! {
                                                <MitigationToggle
                                                    info=m
                                                    on_toggle=Callback::new(move |v: bool| on_toggle.run((id.clone(), v)))
                                                />
                                            }
                                        }).collect::<Vec<_>>()}
                                    </div>
                                </div>
                                <div class="health-section">
                                    <div class="health-section-title">"Live Metrics"</div>
                                    <SnapshotMetrics snapshot=snap />
                                </div>
                            </div>
                        }.into_any()
                    }
                }
            }}
        </div>
    }
}

/// A single mitigation toggle row.
#[component]
fn MitigationToggle(
    info: MitigationInfo,
    on_toggle: Callback<bool>,
) -> impl IntoView {
    let (enabled, set_enabled) = signal(info.enabled);
    let label = info.label.clone();

    view! {
        <div class="health-toggle-row">
            <label class="health-toggle-label">
                <span class=move || {
                    if enabled.get() { "health-dot health-dot-on" } else { "health-dot health-dot-off" }
                } />
                <span>{label}</span>
            </label>
            <button
                class=move || {
                    if enabled.get() { "health-toggle-btn health-toggle-on" }
                    else { "health-toggle-btn health-toggle-off" }
                }
                on:click=move |_| {
                    let new_val = !enabled.get_untracked();
                    set_enabled.set(new_val);
                    on_toggle.run(new_val);
                }
            >
                {move || if enabled.get() { "ON" } else { "OFF" }}
            </button>
        </div>
    }
}

/// Live metrics snapshot display.
#[component]
fn SnapshotMetrics(snapshot: crate::api::health::HealthSnapshot) -> impl IntoView {
    let fd_text = match (snapshot.open_fds, snapshot.fd_limit) {
        (Some(used), Some(limit)) => format!("{} / {}", used, limit),
        _ => "N/A".to_string(),
    };
    let mem_text = match snapshot.memory_rss_bytes {
        Some(b) => format!("{} MB", b / (1024 * 1024)),
        None => "N/A".to_string(),
    };
    let conn_text = match snapshot.tcp_connections {
        Some(c) => c.to_string(),
        None => "N/A".to_string(),
    };

    view! {
        <div class="health-metrics">
            <MetricCard label="File Descriptors" value=fd_text />
            <MetricCard label="RSS Memory" value=mem_text />
            <MetricCard label="TCP Connections" value=conn_text />
            <MetricCard label="Orphan PIDs" value=snapshot.orphan_pids.len().to_string() />
            <MetricCard label="Tracked Ports" value=snapshot.tracked_ports.len().to_string() />
            <MetricCard label="Temp Files" value=snapshot.tracked_temp_files.len().to_string() />
        </div>
    }
}

/// Single metric card.
#[component]
fn MetricCard(label: &'static str, value: String) -> impl IntoView {
    view! {
        <div class="health-metric-card">
            <div class="health-metric-value">{value}</div>
            <div class="health-metric-label">{label}</div>
        </div>
    }
}

/// Audit log tab.
#[component]
fn AuditLogTab(entries: ReadSignal<Vec<AuditEntry>>) -> impl IntoView {
    view! {
        <div class="health-audit">
            {move || {
                let list = entries.get();
                if list.is_empty() {
                    return view! {
                        <div class="health-empty">"No audit entries yet"</div>
                    }.into_any();
                }
                view! {
                    <div class="health-audit-list">
                        {list.into_iter().rev().map(|e| {
                            let cls = if e.success { "health-audit-entry" } else { "health-audit-entry health-audit-fail" };
                            let ts = e.timestamp.chars().take(19).collect::<String>();
                            let action = e.action.clone();
                            let detail = e.detail.clone();
                            view! {
                                <div class=cls>
                                    <span class="health-audit-ts">{ts}</span>
                                    <span class="health-audit-action">{action}</span>
                                    <span class="health-audit-detail">{detail}</span>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                }.into_any()
            }}
        </div>
    }
}
