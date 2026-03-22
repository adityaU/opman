//! Centralized Lucide-compatible icon components.
//! Each icon matches the official Lucide icon SVG paths (lucide.dev).
//! Usage: `<Icon{Name} size=14 />`  (size defaults to 24 if omitted)

use leptos::prelude::*;

/// Shared SVG wrapper — all Lucide icons use 24×24 viewBox, stroke-based.
fn icon_svg(size: u32, class: &'static str, children: impl IntoView) -> impl IntoView {
    let s = format!("{}", size);
    view! {
        <svg
            width=s.clone()
            height=s
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            class=class
        >
            {children}
        </svg>
    }
}

// ── A ───────────────────────────────────────────────────────────────

#[component]
pub fn IconActivity(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! { <path d="M22 12h-2.48a2 2 0 0 0-1.93 1.46l-2.35 8.36a.25.25 0 0 1-.48 0L9.24 2.18a.25.25 0 0 0-.48 0l-2.35 8.36A2 2 0 0 1 4.49 12H2"/> },
    )
}

#[component]
pub fn IconAlertTriangle(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/>
            <path d="M12 9v4"/>
            <path d="M12 17h.01"/>
        },
    )
}

#[component]
pub fn IconArrowUpDown(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="m21 16-4 4-4-4"/><path d="M17 20V4"/>
            <path d="m3 8 4-4 4 4"/><path d="M7 4v16"/>
        },
    )
}

#[component]
pub fn IconAtSign(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <circle cx="12" cy="12" r="4"/>
            <path d="M16 8v5a3 3 0 0 0 6 0v-1a10 10 0 1 0-4 8"/>
        },
    )
}

// ── B ───────────────────────────────────────────────────────────────

#[component]
pub fn IconBookmark(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
    #[prop(default = false)] filled: bool,
) -> impl IntoView {
    let fill = if filled { "currentColor" } else { "none" };
    let s = format!("{}", size);
    view! {
        <svg
            width=s.clone() height=s viewBox="0 0 24 24"
            fill=fill stroke="currentColor" stroke-width="2"
            stroke-linecap="round" stroke-linejoin="round" class=class
        >
            <path d="m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v16z"/>
        </svg>
    }
}

#[component]
pub fn IconBot(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M12 8V4H8"/>
            <rect width="16" height="12" x="4" y="8" rx="2"/>
            <path d="M2 14h2"/><path d="M20 14h2"/>
            <path d="M15 13v2"/><path d="M9 13v2"/>
        },
    )
}

#[component]
pub fn IconBrain(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M12 5a3 3 0 1 0-5.997.125 4 4 0 0 0-2.526 5.77 4 4 0 0 0 .556 6.588A4 4 0 1 0 12 18Z"/>
            <path d="M12 5a3 3 0 1 1 5.997.125 4 4 0 0 1 2.526 5.77 4 4 0 0 1-.556 6.588A4 4 0 1 1 12 18Z"/>
            <path d="M12 5v14"/>
            <path d="M4.54 13.54 12 12"/>
            <path d="M19.46 13.54 12 12"/>
        },
    )
}

// ── C ───────────────────────────────────────────────────────────────

#[component]
pub fn IconCheck(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(size, class, view! { <path d="M20 6 9 17l-5-5"/> })
}

#[component]
pub fn IconChevronDown(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(size, class, view! { <path d="m6 9 6 6 6-6"/> })
}

#[component]
pub fn IconChevronRight(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(size, class, view! { <path d="m9 18 6-6-6-6"/> })
}

#[component]
pub fn IconCommand(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M15 6v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3V6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3"/>
        },
    )
}

#[component]
pub fn IconCopy(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <rect width="14" height="14" x="8" y="8" rx="2" ry="2"/>
            <path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>
        },
    )
}

#[component]
pub fn IconCpu(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <rect width="16" height="16" x="4" y="4" rx="2"/>
            <rect width="6" height="6" x="9" y="9" rx="1"/>
            <path d="M15 2v2"/><path d="M15 20v2"/>
            <path d="M2 15h2"/><path d="M2 9h2"/>
            <path d="M20 15h2"/><path d="M20 9h2"/>
            <path d="M9 2v2"/><path d="M9 20v2"/>
        },
    )
}

// ── D ───────────────────────────────────────────────────────────────

#[component]
pub fn IconDollarSign(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <line x1="12" x2="12" y1="2" y2="22"/>
            <path d="M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6"/>
        },
    )
}

#[component]
pub fn IconDownload(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
            <polyline points="7 10 12 15 17 10"/>
            <line x1="12" x2="12" y1="15" y2="3"/>
        },
    )
}

// ── E ───────────────────────────────────────────────────────────────

#[component]
pub fn IconEye(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0"/>
            <circle cx="12" cy="12" r="3"/>
        },
    )
}

#[component]
pub fn IconExternalLink(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M15 3h6v6"/>
            <path d="M10 14 21 3"/>
            <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>
        },
    )
}

// ── F ───────────────────────────────────────────────────────────────

#[component]
pub fn IconFileCode(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M10 12.5 8 15l2 2.5"/>
            <path d="m14 12.5 2 2.5-2 2.5"/>
            <path d="M14 2v4a2 2 0 0 0 2 2h4"/>
            <path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7z"/>
        },
    )
}

#[component]
pub fn IconFolderPlus(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M12 10v6"/>
            <path d="M9 13h6"/>
            <path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2z"/>
        },
    )
}

// ── G ───────────────────────────────────────────────────────────────

#[component]
pub fn IconGitBranch(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <line x1="6" x2="6" y1="3" y2="15"/>
            <circle cx="18" cy="6" r="3"/>
            <circle cx="6" cy="18" r="3"/>
            <path d="M18 9a9 9 0 0 1-9 9"/>
        },
    )
}

// ── H ───────────────────────────────────────────────────────────────

#[component]
pub fn IconHardDrive(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <line x1="22" x2="2" y1="12" y2="12"/>
            <path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/>
            <line x1="6" x2="6.01" y1="16" y2="16"/>
            <line x1="10" x2="10.01" y1="16" y2="16"/>
        },
    )
}

// ── I ───────────────────────────────────────────────────────────────

#[component]
pub fn IconImage(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <rect width="18" height="18" x="3" y="3" rx="2" ry="2"/>
            <circle cx="9" cy="9" r="2"/>
            <path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/>
        },
    )
}

// ── L ───────────────────────────────────────────────────────────────

#[component]
pub fn IconLayers(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="m12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83Z"/>
            <path d="m22 17.65-9.17 4.16a2 2 0 0 1-1.66 0L2 17.65"/>
            <path d="m22 12.65-9.17 4.16a2 2 0 0 1-1.66 0L2 12.65"/>
        },
    )
}

#[component]
pub fn IconLoader2(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    let cls = if class.is_empty() { "spinning" } else { class };
    icon_svg(size, cls, view! { <path d="M21 12a9 9 0 1 1-6.219-8.56"/> })
}

// ── M ───────────────────────────────────────────────────────────────

#[component]
pub fn IconMenu(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <line x1="4" x2="20" y1="12" y2="12"/>
            <line x1="4" x2="20" y1="6" y2="6"/>
            <line x1="4" x2="20" y1="18" y2="18"/>
        },
    )
}

#[component]
pub fn IconMessageCircle(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M7.9 20A9 9 0 1 0 4 16.1L2 22Z"/>
        },
    )
}

#[component]
pub fn IconMoreHorizontal(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <circle cx="12" cy="12" r="1"/>
            <circle cx="19" cy="12" r="1"/>
            <circle cx="5" cy="12" r="1"/>
        },
    )
}

#[component]
pub fn IconMoreVertical(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <circle cx="12" cy="12" r="1"/>
            <circle cx="12" cy="5" r="1"/>
            <circle cx="12" cy="19" r="1"/>
        },
    )
}

// ── N ───────────────────────────────────────────────────────────────

#[component]
pub fn IconNetwork(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <rect x="16" y="16" width="6" height="6" rx="1"/>
            <rect x="2" y="16" width="6" height="6" rx="1"/>
            <rect x="9" y="2" width="6" height="6" rx="1"/>
            <path d="M5 16v-3a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v3"/>
            <path d="M12 12V8"/>
        },
    )
}

// ── P ───────────────────────────────────────────────────────────────

#[component]
pub fn IconPanelLeft(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <rect width="18" height="18" x="3" y="3" rx="2"/>
            <path d="M9 3v18"/>
        },
    )
}

#[component]
pub fn IconPaperclip(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="m21.44 11.05-9.19 9.19a6 6 0 0 1-8.49-8.49l8.57-8.57A4 4 0 1 1 18 8.84l-8.59 8.57a2 2 0 0 1-2.83-2.83l8.49-8.48"/>
        },
    )
}

#[component]
pub fn IconPencil(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z"/>
            <path d="m15 5 4 4"/>
        },
    )
}

#[component]
pub fn IconPenSquare(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/>
            <path d="M18.5 2.5a2.12 2.12 0 0 1 3 3L12 15l-4 1 1-4Z"/>
        },
    )
}

#[component]
pub fn IconPin(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    let s = format!("{}", size);
    view! {
        <svg width=s.clone() height=s viewBox="0 0 24 24" fill="currentColor" stroke="none" class=class>
            <path d="M16 12V4h1V2H7v2h1v8l-2 2v2h5.2v6h1.6v-6H18v-2l-2-2z"/>
        </svg>
    }
}

#[component]
pub fn IconPlus(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M5 12h14"/>
            <path d="M12 5v14"/>
        },
    )
}

// ── R ───────────────────────────────────────────────────────────────

#[component]
pub fn IconRotateCcw(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/>
            <path d="M3 3v5h5"/>
        },
    )
}

// ── S ───────────────────────────────────────────────────────────────

#[component]
pub fn IconSearch(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <circle cx="11" cy="11" r="8"/>
            <path d="m21 21-4.3-4.3"/>
        },
    )
}

#[component]
pub fn IconSend(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M14.536 21.686a.5.5 0 0 0 .937-.024l6.5-19a.496.496 0 0 0-.635-.635l-19 6.5a.5.5 0 0 0-.024.937l7.93 3.18a2 2 0 0 1 1.112 1.11z"/>
            <path d="m21.854 2.147-10.94 10.939"/>
        },
    )
}

#[component]
pub fn IconSparkles(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z"/>
            <path d="M20 3v4"/><path d="M22 5h-4"/>
            <path d="M4 17v2"/><path d="M5 18H3"/>
        },
    )
}

#[component]
pub fn IconSquare(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    let s = format!("{}", size);
    view! {
        <svg width=s.clone() height=s viewBox="0 0 24 24" fill="currentColor" stroke="none" class=class>
            <rect width="14" height="14" x="5" y="5" rx="2"/>
        </svg>
    }
}

// ── T ───────────────────────────────────────────────────────────────

#[component]
pub fn IconTerminal(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <polyline points="4,17 10,11 4,5"/>
            <line x1="12" x2="20" y1="19" y2="19"/>
        },
    )
}

#[component]
pub fn IconTrash2(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/>
            <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/>
            <line x1="10" x2="10" y1="11" y2="17"/>
            <line x1="14" x2="14" y1="11" y2="17"/>
        },
    )
}

// ── U ───────────────────────────────────────────────────────────────

#[component]
pub fn IconUser(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/>
            <circle cx="12" cy="7" r="4"/>
        },
    )
}

#[component]
pub fn IconUsers(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/>
            <circle cx="9" cy="7" r="4"/>
            <path d="M22 21v-2a4 4 0 0 0-3-3.87"/>
            <path d="M16 3.13a4 4 0 0 1 0 7.75"/>
        },
    )
}

// ── W ───────────────────────────────────────────────────────────────

#[component]
pub fn IconWifi(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M12 20h.01"/>
            <path d="M8.5 16.429a5 5 0 0 1 7 0"/>
            <path d="M5 12.859a10 10 0 0 1 14 0"/>
            <path d="M2 8.82a15 15 0 0 1 20 0"/>
        },
    )
}

#[component]
pub fn IconWifiOff(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M12 20h.01"/>
            <path d="M8.5 16.429a5 5 0 0 1 7 0"/>
            <path d="M5 12.859a10 10 0 0 1 5.17-2.69"/>
            <path d="M13.83 10.17A10 10 0 0 1 19 12.859"/>
            <path d="M2 8.82a15 15 0 0 1 4.17-2.65"/>
            <path d="M10.66 5c4.01-.36 8.14.9 11.34 3.76"/>
            <line x1="2" x2="22" y1="2" y2="22"/>
        },
    )
}

#[component]
pub fn IconWrapText(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <line x1="3" x2="21" y1="6" y2="6"/>
            <path d="M3 12h15a3 3 0 1 1 0 6h-4"/>
            <polyline points="16 16 14 18 16 20"/>
            <line x1="3" x2="10" y1="18" y2="18"/>
        },
    )
}

#[component]
pub fn IconWrench(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/>
        },
    )
}

// ── X ───────────────────────────────────────────────────────────────

#[component]
pub fn IconX(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M18 6 6 18"/>
            <path d="m6 6 12 12"/>
        },
    )
}

// ── Z ───────────────────────────────────────────────────────────────

#[component]
pub fn IconZap(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M4 14a1 1 0 0 1-.78-1.63l9.9-10.2a.5.5 0 0 1 .86.46l-1.92 6.02A1 1 0 0 0 13 10h7a1 1 0 0 1 .78 1.63l-9.9 10.2a.5.5 0 0 1-.86-.46l1.92-6.02A1 1 0 0 0 11 14z"/>
        },
    )
}

// ── Additional icons for ToolCall parity ─────────────────────────────

#[component]
pub fn IconCheckCircle2(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <circle cx="12" cy="12" r="10"/>
            <path d="m9 12 2 2 4-4"/>
        },
    )
}

#[component]
pub fn IconXCircle(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <circle cx="12" cy="12" r="10"/>
            <path d="m15 9-6 6"/>
            <path d="m9 9 6 6"/>
        },
    )
}

#[component]
pub fn IconClock(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <circle cx="12" cy="12" r="10"/>
            <polyline points="12 6 12 12 16 14"/>
        },
    )
}

#[component]
pub fn IconCircleDot(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <circle cx="12" cy="12" r="10"/>
            <circle cx="12" cy="12" r="1"/>
        },
    )
}

#[component]
pub fn IconMinus(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M5 12h14"/>
        },
    )
}

#[component]
pub fn IconCircle(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <circle cx="12" cy="12" r="10"/>
        },
    )
}

// ── Additional icons for CodeEditorPanel (toolbar parity) ───────────

#[component]
pub fn IconAlertCircle(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <circle cx="12" cy="12" r="10"/>
            <line x1="12" x2="12" y1="8" y2="12"/>
            <line x1="12" x2="12.01" y1="16" y2="16"/>
        },
    )
}

#[component]
pub fn IconWand2(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="m21.64 3.64-1.28-1.28a1.21 1.21 0 0 0-1.72 0L2.36 18.64a1.21 1.21 0 0 0 0 1.72l1.28 1.28a1.2 1.2 0 0 0 1.72 0L21.64 5.36a1.2 1.2 0 0 0 0-1.72"/>
            <path d="m14 7 3 3"/>
            <path d="M5 6v4"/><path d="M19 14v4"/>
            <path d="M10 2v2"/><path d="M7 8H3"/><path d="M21 16h-4"/>
            <path d="M11 3H9"/>
        },
    )
}

#[component]
pub fn IconInfo(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <circle cx="12" cy="12" r="10"/>
            <path d="M12 16v-4"/>
            <path d="M12 8h.01"/>
        },
    )
}

#[component]
pub fn IconArrowRightCircle(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <circle cx="12" cy="12" r="10"/>
            <path d="M8 12h8"/>
            <path d="m12 16 4-4-4-4"/>
        },
    )
}

#[component]
pub fn IconUpload(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
            <polyline points="17 8 12 3 7 8"/>
            <line x1="12" x2="12" y1="3" y2="15"/>
        },
    )
}

#[component]
pub fn IconFilePlus(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/>
            <path d="M14 2v4a2 2 0 0 0 2 2h4"/>
            <path d="M9 15h6"/>
            <path d="M12 18v-6"/>
        },
    )
}

#[component]
pub fn IconFolder(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2z"/>
        },
    )
}

#[component]
pub fn IconPanelLeftClose(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <rect width="18" height="18" x="3" y="3" rx="2"/>
            <path d="M9 3v18"/>
            <path d="m16 15-3-3 3-3"/>
        },
    )
}

#[component]
pub fn IconPanelLeftOpen(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <rect width="18" height="18" x="3" y="3" rx="2"/>
            <path d="M9 3v18"/>
            <path d="m14 9 3 3-3 3"/>
        },
    )
}

#[component]
pub fn IconChevronDown2(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(size, class, view! { <path d="m6 9 6 6 6-6"/> })
}

// ── Additional icons for CodeEditorPanel ─────────────────────────────

#[component]
pub fn IconCode2(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="m18 16 4-4-4-4"/>
            <path d="m6 8-4 4 4 4"/>
            <path d="m14.5 4-5 16"/>
        },
    )
}

#[component]
pub fn IconSave(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M15.2 3a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z"/>
            <path d="M17 21v-7a1 1 0 0 0-1-1H8a1 1 0 0 0-1 1v7"/>
            <path d="M7 3v4a1 1 0 0 0 1 1h7"/>
        },
    )
}

#[component]
pub fn IconFile(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/>
            <path d="M14 2v4a2 2 0 0 0 2 2h4"/>
        },
    )
}

#[component]
pub fn IconChevronLeft(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="m15 18-6-6 6-6"/>
        },
    )
}

#[component]
pub fn IconGitPullRequest(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <circle cx="18" cy="18" r="3"/>
            <circle cx="6" cy="6" r="3"/>
            <path d="M13 6h3a2 2 0 0 1 2 2v7"/>
            <path d="M6 9v12"/>
        },
    )
}

#[component]
pub fn IconMessageSquare(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
        },
    )
}

#[component]
pub fn IconFileEdit(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M12 22h6a2 2 0 0 0 2-2V7l-5-5H6a2 2 0 0 0-2 2v10"/>
            <path d="M14 2v4a2 2 0 0 0 2 2h4"/>
            <path d="M10.4 12.6a2 2 0 0 0-3 3L12 20l4 1-1-4Z"/>
        },
    )
}

// ── Additional icons (GitFork, History, GitCommitHorizontal, RefreshCw, FileText) ──

#[component]
pub fn IconGitFork(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <circle cx="12" cy="18" r="3"/>
            <circle cx="6" cy="6" r="3"/>
            <circle cx="18" cy="6" r="3"/>
            <path d="M18 9v2c0 .6-.4 1-1 1H7c-.6 0-1-.4-1-1V9"/>
            <path d="M12 12v3"/>
        },
    )
}

#[component]
pub fn IconHistory(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/>
            <path d="M3 3v5h5"/>
            <path d="M12 7v5l4 2"/>
        },
    )
}

#[component]
pub fn IconGitCommitHorizontal(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <circle cx="12" cy="12" r="3"/>
            <line x1="3" x2="9" y1="12" y2="12"/>
            <line x1="15" x2="21" y1="12" y2="12"/>
        },
    )
}

#[component]
pub fn IconRefreshCw(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/>
            <path d="M21 3v5h-5"/>
            <path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/>
            <path d="M3 21v-5h5"/>
        },
    )
}

#[component]
pub fn IconFileText(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/>
            <path d="M14 2v4a2 2 0 0 0 2 2h4"/>
            <path d="M10 9H8"/>
            <path d="M16 13H8"/>
            <path d="M16 17H8"/>
        },
    )
}

// ── Maximize / Minimize ─────────────────────────────────────────────

#[component]
pub fn IconMaximize2(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <polyline points="15 3 21 3 21 9"/>
            <polyline points="9 21 3 21 3 15"/>
            <line x1="21" x2="14" y1="3" y2="10"/>
            <line x1="3" x2="10" y1="21" y2="14"/>
        },
    )
}

#[component]
pub fn IconMinimize2(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <polyline points="4 14 10 14 10 20"/>
            <polyline points="20 10 14 10 14 4"/>
            <line x1="14" x2="21" y1="10" y2="3"/>
            <line x1="3" x2="10" y1="21" y2="14"/>
        },
    )
}

#[component]
pub fn IconHelpCircle(
    #[prop(default = 24)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    icon_svg(
        size,
        class,
        view! {
            <circle cx="12" cy="12" r="10"/>
            <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/>
            <line x1="12" x2="12.01" y1="17" y2="17"/>
        },
    )
}
