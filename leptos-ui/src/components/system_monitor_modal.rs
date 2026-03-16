//! SystemMonitorModal — live system resource monitor with SSE streaming.
//! Matches React `SystemMonitorModal.tsx` structure and `sysm-*` CSS classes.
//! Connects to `/api/system/stats/stream` via EventSource for live data.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use crate::components::icons::*;
use crate::hooks::use_focus_trap::use_focus_trap;
use crate::types::api::SystemStats;

// ── Helpers ─────────────────────────────────────────────────────────

fn fmt_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".into();
    }
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let i = ((bytes as f64).ln() / 1024_f64.ln()).floor() as usize;
    let i = i.min(UNITS.len() - 1);
    let val = bytes as f64 / 1024_f64.powi(i as i32);
    if val < 10.0 {
        format!("{:.1} {}", val, UNITS[i])
    } else {
        format!("{:.0} {}", val, UNITS[i])
    }
}

fn fmt_uptime(secs: u64) -> String {
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let mut parts = Vec::new();
    if d > 0 { parts.push(format!("{}d", d)); }
    if h > 0 { parts.push(format!("{}h", h)); }
    parts.push(format!("{}m", m));
    parts.join(" ")
}

fn pct(used: u64, total: u64) -> f64 {
    if total > 0 { (used as f64 / total as f64) * 100.0 } else { 0.0 }
}

fn fmt_pct(p: f64) -> String {
    format!("{:.1}%", p)
}

fn bar_level(p: f64) -> &'static str {
    if p >= 90.0 { "critical" }
    else if p >= 70.0 { "high" }
    else { "" }
}

fn cpu_core_level(usage: f64) -> &'static str {
    if usage >= 90.0 { "sysm-core-burning" }
    else if usage >= 60.0 { "sysm-core-hot" }
    else { "" }
}

fn cpu_cell_class(cpu: f64) -> &'static str {
    if cpu >= 80.0 { "sysm-cell-critical" }
    else if cpu >= 30.0 { "sysm-cell-warn" }
    else { "" }
}

// ── Column definitions ──────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortKey {
    Pid,
    Name,
    Cpu,
    Mem,
    DiskRead,
    DiskWrite,
    Status,
}

impl SortKey {
    fn label(self) -> &'static str {
        match self {
            Self::Pid => "PID",
            Self::Name => "Name",
            Self::Cpu => "CPU",
            Self::Mem => "Mem",
            Self::DiskRead => "IO R",
            Self::DiskWrite => "IO W",
            Self::Status => "State",
        }
    }

    fn is_right(self) -> bool {
        matches!(self, Self::Pid | Self::Cpu | Self::Mem | Self::DiskRead | Self::DiskWrite)
    }

    fn default_dir(self) -> bool {
        // true = ascending default (name, status); false = descending default
        matches!(self, Self::Name | Self::Status)
    }
}

const ALL_COLS: &[SortKey] = &[
    SortKey::Pid,
    SortKey::Name,
    SortKey::Cpu,
    SortKey::Mem,
    SortKey::DiskRead,
    SortKey::DiskWrite,
    SortKey::Status,
];

// ── Component ───────────────────────────────────────────────────────

#[component]
pub fn SystemMonitorModal(
    on_close: Callback<()>,
) -> impl IntoView {
    let modal_ref = NodeRef::<leptos::html::Div>::new();
    use_focus_trap(modal_ref);

    let (stats, set_stats) = signal(Option::<SystemStats>::None);
    let (connected, set_connected) = signal(false);
    let (error, set_error) = signal(false);
    let (cores_open, set_cores_open) = signal(false);
    let (sort_key, set_sort_key) = signal(SortKey::Cpu);
    let (sort_asc, set_sort_asc) = signal(false); // false = desc

    // Escape key handler
    {
        let on_close = on_close.clone();
        leptos::task::spawn_local(async move {
            use wasm_bindgen::closure::Closure;
            let cb = Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |ev: web_sys::KeyboardEvent| {
                if ev.key() == "Escape" {
                    on_close.run(());
                }
            });
            if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                let _ = doc.add_event_listener_with_callback("keydown", cb.as_ref().unchecked_ref());
            }
            cb.forget();
        });
    }

    // Connect to SSE stream on mount
    {
        leptos::task::spawn_local(async move {
            let es = web_sys::EventSource::new("/api/system/stats/stream").ok();
            if let Some(event_source) = es {
                if !connected.get_untracked() {
                    set_connected.set(true);
                }

                let on_message = Closure::<dyn Fn(web_sys::MessageEvent)>::new(move |ev: web_sys::MessageEvent| {
                    if let Some(data) = ev.data().as_string() {
                        if let Ok(parsed) = serde_json::from_str::<SystemStats>(&data) {
                            set_stats.set(Some(parsed));
                            if !connected.get_untracked() {
                                set_connected.set(true);
                            }
                            if error.get_untracked() {
                                set_error.set(false);
                            }
                        }
                    }
                });

                let on_named = Closure::<dyn Fn(web_sys::MessageEvent)>::new(move |ev: web_sys::MessageEvent| {
                    if let Some(data) = ev.data().as_string() {
                        if let Ok(parsed) = serde_json::from_str::<SystemStats>(&data) {
                            set_stats.set(Some(parsed));
                            if !connected.get_untracked() {
                                set_connected.set(true);
                            }
                            if error.get_untracked() {
                                set_error.set(false);
                            }
                        }
                    }
                });

                let on_err = Closure::<dyn Fn()>::new(move || {
                    if !error.get_untracked() {
                        set_error.set(true);
                    }
                });

                event_source.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
                event_source.set_onerror(Some(on_err.as_ref().unchecked_ref()));
                event_source.add_event_listener_with_callback(
                    "system_stats",
                    on_named.as_ref().unchecked_ref(),
                ).ok();

                on_message.forget();
                on_named.forget();
                on_err.forget();

                // Close EventSource on component cleanup instead of leaking it
                on_cleanup(move || {
                    event_source.close();
                });
            }
        });
    }

    let handle_sort = move |col: SortKey| {
        move |_: web_sys::MouseEvent| {
            let cur = sort_key.get_untracked();
            if cur == col {
                set_sort_asc.update(|v| *v = !*v);
            } else {
                set_sort_key.set(col);
                set_sort_asc.set(col.default_dir());
            }
        }
    };

    view! {
        <div class="sysm-overlay" on:click=move |_| on_close.run(())>
            <div
                class="sysm-modal"
                on:click=move |e| e.stop_propagation()
                tabindex="0"
                role="dialog"
                aria-modal="true"
                aria-label="System Monitor"
                node_ref=modal_ref
            >
                // ── Header ──────────────────────────────
                <div class="sysm-header">
                    <div class="sysm-header-left">
                        <IconActivity size=16 />
                        <h3>"System Monitor"</h3>
                        {move || stats.get().map(|s| {
                            let hn = s.hostname.clone();
                            view! { <span class="sysm-host">{hn}</span> }
                        })}
                        {move || stats.get().map(|s| {
                            let up = fmt_uptime(s.uptime_secs);
                            view! { <span class="sysm-badge">{"up "}{up}</span> }
                        })}
                    </div>
                    <div class="sysm-header-right">
                        <span class="sysm-status">
                            <span class=move || {
                                if error.get() { "sysm-dot error" } else { "sysm-dot" }
                            }/>
                            {move || {
                                if connected.get() {
                                    if error.get() { "Error" } else { "Live" }
                                } else if error.get() {
                                    "Error"
                                } else {
                                    "..."
                                }
                            }}
                        </span>
                        <button on:click=move |_| on_close.run(()) aria-label="Close">
                            <IconX size=16 />
                        </button>
                    </div>
                </div>

                // ── Body ────────────────────────────────
                <div class="sysm-body">
                    {move || {
                        let s = match stats.get() {
                            Some(s) => s,
                            None => return view! {
                                <div class="sysm-empty">"Connecting to system monitor..."</div>
                            }.into_any(),
                        };

                        let cpu_pct = s.cpu_avg;
                        let mem_pct = pct(s.mem_used, s.mem_total);
                        let swap_pct = pct(s.swap_used, s.swap_total);
                        let has_swap = s.swap_total > 0;

                        // ── Overview gauges ─────────────
                        let cpu_level = bar_level(cpu_pct);
                        let mem_level = bar_level(mem_pct);
                        let swap_level = bar_level(swap_pct);

                        let gauges = view! {
                            <div class="sysm-gauges">
                                // CPU gauge
                                <div class="sysm-gauge">
                                    <div class="sysm-gauge-top">
                                        <span class="sysm-gauge-label">"CPU"</span>
                                        <span class="sysm-gauge-val">{fmt_pct(cpu_pct)}</span>
                                    </div>
                                    <div class="sysm-bar-track">
                                        <div
                                            class=format!("sysm-bar-fill info {}", cpu_level)
                                            style=format!("width: {:.0}%", cpu_pct.min(100.0))
                                        />
                                    </div>
                                </div>
                                // Memory gauge
                                <div class="sysm-gauge">
                                    <div class="sysm-gauge-top">
                                        <span class="sysm-gauge-label">"Memory"</span>
                                        <span class="sysm-gauge-val">
                                            {format!("{} / {}", fmt_bytes(s.mem_used), fmt_bytes(s.mem_total))}
                                        </span>
                                    </div>
                                    <div class="sysm-bar-track">
                                        <div
                                            class=format!("sysm-bar-fill success {}", mem_level)
                                            style=format!("width: {:.0}%", mem_pct.min(100.0))
                                        />
                                    </div>
                                </div>
                                // Swap gauge (conditional)
                                {has_swap.then(|| view! {
                                    <div class="sysm-gauge">
                                        <div class="sysm-gauge-top">
                                            <span class="sysm-gauge-label">"Swap"</span>
                                            <span class="sysm-gauge-val">
                                                {format!("{} / {}", fmt_bytes(s.swap_used), fmt_bytes(s.swap_total))}
                                            </span>
                                        </div>
                                        <div class="sysm-bar-track">
                                            <div
                                                class=format!("sysm-bar-fill warning {}", swap_level)
                                                style=format!("width: {:.0}%", swap_pct.min(100.0))
                                            />
                                        </div>
                                    </div>
                                })}
                            </div>
                        };

                        // ── Stats row (load + process count + networks) ──
                        let (l1, l5, l15) = s.load_avg;
                        let proc_count_text = format!("{} processes", s.process_count);
                        let net_spans = s.networks.iter().take(3).map(|n| {
                            let name = n.name.clone();
                            let rx = fmt_bytes(n.rx_bytes);
                            let tx = fmt_bytes(n.tx_bytes);
                            view! {
                                <span class="sysm-stat sysm-stat-net">
                                    <IconNetwork size=10 />
                                    <span class="sysm-net-name">{name}</span>
                                    <span class="sysm-net-rx">{rx}</span>
                                    <span class="sysm-net-sep">"/"</span>
                                    <span class="sysm-net-tx">{tx}</span>
                                </span>
                            }
                        }).collect::<Vec<_>>();

                        let stats_row = view! {
                            <div class="sysm-stats-row">
                                <span class="sysm-stat">
                                    "Load "
                                    <strong>{format!("{:.2}", l1)}</strong>
                                    {format!(" / {:.2} / {:.2}", l5, l15)}
                                </span>
                                <span class="sysm-stat">{proc_count_text}</span>
                                {net_spans}
                            </div>
                        };

                        // ── CPU cores (collapsible) ────────
                        let cores = s.cpu_usage.clone();
                        let num_cores = cores.len();
                        let cores_section = if num_cores > 1 {
                            let cores_open_val = cores_open.get_untracked();
                            let cores_inner = if cores_open_val {
                                let core_bars = cores.iter().enumerate().map(|(i, &usage)| {
                                    let level = cpu_core_level(usage);
                                    let fill_level = bar_level(usage);
                                    let cls = format!("sysm-core {}", level);
                                    let fill_cls = format!("sysm-core-fill {}", fill_level);
                                    view! {
                                        <div class=cls title=format!("Core #{}: {:.1}%", i, usage)>
                                            <span class="sysm-core-id">{i}</span>
                                            <div class="sysm-core-bar">
                                                <div
                                                    class=fill_cls
                                                    style=format!("height: {:.0}%", usage.min(100.0))
                                                />
                                            </div>
                                            <span class="sysm-core-val">{format!("{:.0}", usage)}</span>
                                        </div>
                                    }
                                }).collect::<Vec<_>>();
                                Some(view! { <div class="sysm-cores">{core_bars}</div> })
                            } else {
                                None
                            };

                            Some(view! {
                                <div class="sysm-section">
                                    <button
                                        class="sysm-section-toggle"
                                        on:click=move |_| set_cores_open.update(|v| *v = !*v)
                                    >
                                        {move || if cores_open.get() {
                                            view! { <IconChevronDown size=12 /> }.into_any()
                                        } else {
                                            view! { <IconChevronRight size=12 /> }.into_any()
                                        }}
                                        <IconCpu size=11 />
                                        <span>{format!("CPU Cores ({})", num_cores)}</span>
                                    </button>
                                    {cores_inner}
                                </div>
                            })
                        } else {
                            None
                        };

                        // ── Process table ──────────────────
                        let mut procs = s.processes.clone();
                        let cur_key = sort_key.get_untracked();
                        let cur_asc = sort_asc.get_untracked();
                        let mul: i32 = if cur_asc { 1 } else { -1 };
                        procs.sort_by(|a, b| {
                            let cmp = match cur_key {
                                SortKey::Pid => a.pid.cmp(&b.pid),
                                SortKey::Name => a.name.cmp(&b.name),
                                SortKey::Cpu => a.cpu.partial_cmp(&b.cpu).unwrap_or(std::cmp::Ordering::Equal),
                                SortKey::Mem => a.mem.cmp(&b.mem),
                                SortKey::DiskRead => a.disk_read.cmp(&b.disk_read),
                                SortKey::DiskWrite => a.disk_write.cmp(&b.disk_write),
                                SortKey::Status => a.status.cmp(&b.status),
                            };
                            if mul < 0 { cmp.reverse() } else { cmp }
                        });

                        let header_cells = ALL_COLS.iter().map(|&col| {
                            let active = cur_key == col;
                            let arrow = if active {
                                if cur_asc { " \u{2191}" } else { " \u{2193}" }
                            } else {
                                ""
                            };
                            let cls = format!(
                                "{}{}",
                                if col.is_right() { "r " } else { "" },
                                if active { "active" } else { "" }
                            );
                            view! {
                                <th class=cls on:click=handle_sort(col)>
                                    {col.label()}{arrow}
                                </th>
                            }
                        }).collect::<Vec<_>>();

                        let proc_rows = procs.iter().take(50).map(|p| {
                            let pid = p.pid;
                            let name = p.name.clone();
                            let name_title = name.clone();
                            let cpu = p.cpu;
                            let mem = p.mem;
                            let dr = p.disk_read;
                            let dw = p.disk_write;
                            let st = p.status.clone();
                            let cpu_cls = cpu_cell_class(cpu);
                            view! {
                                <tr>
                                    <td class="r">{pid}</td>
                                    <td><span title=name_title>{name}</span></td>
                                    <td class="r"><span class=cpu_cls>{format!("{:.1}%", cpu)}</span></td>
                                    <td class="r">{fmt_bytes(mem)}</td>
                                    <td class="r">{fmt_bytes(dr)}</td>
                                    <td class="r">{fmt_bytes(dw)}</td>
                                    <td>{st}</td>
                                </tr>
                            }
                        }).collect::<Vec<_>>();

                        let proc_section = view! {
                            <div class="sysm-section">
                                <div class="sysm-section-label">
                                    <IconArrowUpDown size=11 />
                                    <span>"Processes"</span>
                                </div>
                                <div class="sysm-table-wrap">
                                    <table class="sysm-table">
                                        <thead>
                                            <tr>{header_cells}</tr>
                                        </thead>
                                        <tbody>
                                            {proc_rows}
                                        </tbody>
                                    </table>
                                </div>
                            </div>
                        };

                        // ── Disks ─────────────────────────
                        let disks_section = if !s.disks.is_empty() {
                            let disk_cards = s.disks.iter().map(|d| {
                                let dp = pct(d.used, d.total);
                                let level = bar_level(dp);
                                let mount_or_name = if d.mount.is_empty() { d.name.clone() } else { d.mount.clone() };
                                let detail = format!("{} / {}", fmt_bytes(d.used), fmt_bytes(d.total));
                                let fs = d.fs_type.clone();
                                let has_fs = !fs.is_empty();
                                view! {
                                    <div class="sysm-disk">
                                        <div class="sysm-disk-top">
                                            <span class="sysm-disk-name">{mount_or_name}</span>
                                            <span class="sysm-disk-pct">{fmt_pct(dp)}</span>
                                        </div>
                                        <div class="sysm-bar-track">
                                            <div
                                                class=format!("sysm-bar-fill disk {}", level)
                                                style=format!("width: {:.0}%", dp.min(100.0))
                                            />
                                        </div>
                                        <div class="sysm-disk-detail">
                                            {detail}
                                            {has_fs.then(|| view! {
                                                <span class="sysm-disk-fs">{fs}</span>
                                            })}
                                        </div>
                                    </div>
                                }
                            }).collect::<Vec<_>>();
                            Some(view! {
                                <div class="sysm-section">
                                    <div class="sysm-section-label">
                                        <IconHardDrive size=11 />
                                        <span>"Disks"</span>
                                    </div>
                                    <div class="sysm-disks">
                                        {disk_cards}
                                    </div>
                                </div>
                            })
                        } else {
                            None
                        };

                        view! {
                            <div>
                                {gauges}
                                {stats_row}
                                {cores_section}
                                {proc_section}
                                {disks_section}
                            </div>
                        }.into_any()
                    }}
                </div>

                // ── Footer ──────────────────────────────
                <div class="sysm-footer">
                    <kbd>"Esc"</kbd>" Close"
                    <span class="sysm-footer-sep"/>
                    "Click column headers to sort"
                </div>
            </div>
        </div>
    }
}
