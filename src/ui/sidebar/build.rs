use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;

use super::lerp_color;
use super::Sidebar;

/// Session indicator priority (highest first):
/// 1. busy → pulsating accent dot
/// 2. input-needed → warning dot
/// 3. error → red dot
/// 4. unseen → blue/info dot
/// 5. idle → subtle dash
enum Indicator {
    Busy,
    Input,
    Error,
    Unseen,
    Idle,
}

impl<'a> Sidebar<'a> {
    /// Determine the highest-priority indicator state for a session.
    fn session_indicator(&self, session_id: &str) -> Indicator {
        if self.app.active_sessions.contains(session_id) {
            return Indicator::Busy;
        }
        // Check subagents — if any child is busy, parent shows busy
        for (_, project) in self.app.projects.iter().enumerate() {
            if let Some(session) = project.sessions.iter().find(|s| s.id == session_id) {
                let subs = self.app.session_children.get(&session.id);
                if let Some(children) = subs {
                    if children
                        .iter()
                        .any(|cid| self.app.active_sessions.contains(cid))
                    {
                        return Indicator::Busy;
                    }
                }
                break;
            }
        }
        if self.app.input_sessions.contains(session_id) {
            return Indicator::Input;
        }
        if self.app.error_sessions.contains(session_id) {
            return Indicator::Error;
        }
        if self.app.unseen_sessions.contains(session_id) {
            return Indicator::Unseen;
        }
        Indicator::Idle
    }

    /// Render the indicator span for a session, respecting cursor background.
    fn indicator_span(&self, indicator: &Indicator, is_cursor: bool) -> Span<'a> {
        match indicator {
            Indicator::Busy => {
                let dot_color = lerp_color(
                    self.app.theme.background,
                    self.app.theme.accent,
                    self.app.pulse_phase,
                );
                let mut style = Style::default().fg(dot_color);
                if is_cursor {
                    style = self.with_cursor_bg(style);
                }
                Span::styled("● ", style)
            }
            Indicator::Input => {
                let mut style = Style::default().fg(self.app.theme.warning);
                if is_cursor {
                    style = self.with_cursor_bg(style);
                }
                Span::styled("● ", style)
            }
            Indicator::Error => {
                let mut style = Style::default().fg(self.app.theme.error);
                if is_cursor {
                    style = self.with_cursor_bg(style);
                }
                Span::styled("● ", style)
            }
            Indicator::Unseen => {
                let mut style = Style::default().fg(self.app.theme.info);
                if is_cursor {
                    style = self.with_cursor_bg(style);
                }
                Span::styled("● ", style)
            }
            Indicator::Idle => {
                let mut style = Style::default().fg(self.app.theme.text_muted);
                if is_cursor {
                    style = self.with_cursor_bg(style);
                }
                Span::styled("─ ", style)
            }
        }
    }

    /// Build the list items for the project list.
    ///
    /// Two independent highlights:
    ///   - **cursor** (`sidebar_cursor`): background highlight via `background_element`
    ///   - **selected** (`sidebar_selection`): bold + primary foreground (active session)
    pub(super) fn build_items(&self) -> Vec<ListItem<'a>> {
        let mut items = Vec::new();

        if self.app.projects.is_empty() {
            items.push(ListItem::new(Line::from(vec![Span::styled(
                "  No projects.",
                Style::default().fg(self.app.theme.text_muted),
            )])));
            items.push(ListItem::new(Line::from(vec![Span::styled(
                "  Press 'a' to add one.",
                Style::default().fg(self.app.theme.text_muted),
            )])));
            return items;
        }

        let mut flat_idx = 0;

        for (i, project) in self.app.projects.iter().enumerate() {
            let is_active = i == self.app.active_project;
            let is_selected = flat_idx == self.app.sidebar_selection;
            let is_cursor = flat_idx == self.app.sidebar_cursor;

            let marker = if is_active { "▶ " } else { "  " };
            let mut style = if is_selected {
                Style::default()
                    .fg(self.app.theme.primary)
                    .add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default().fg(self.app.theme.success)
            } else {
                Style::default().fg(self.app.theme.text)
            };
            if is_cursor {
                style = self.with_cursor_bg(style);
            }

            // Project-level indicator: highest priority across all sessions
            let project_indicator = self.project_indicator(i);

            let mut spans = vec![Span::styled(marker, style)];
            if !matches!(project_indicator, Indicator::Idle) {
                spans.push(self.indicator_span(&project_indicator, is_cursor));
            }
            spans.push(Span::styled(project.name.clone(), style));
            let project_line = Line::from(spans);
            items.push(ListItem::new(project_line));
            flat_idx += 1;

            let is_expanded = self.app.sessions_expanded_for == Some(i);
            let visible = self.app.visible_sessions(i);

            if is_expanded {
                let is_sel = flat_idx == self.app.sidebar_selection;
                let is_cur = flat_idx == self.app.sidebar_cursor;
                let mut ns_style = if is_sel {
                    Style::default()
                        .fg(self.app.theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(self.app.theme.accent)
                };
                if is_cur {
                    ns_style = self.with_cursor_bg(ns_style);
                }
                let mut pad_style = Style::default();
                if is_cur {
                    pad_style = self.with_cursor_bg(pad_style);
                }
                items.push(ListItem::new(Line::from(vec![
                    Span::styled("    ", pad_style),
                    Span::styled("+ New Session", ns_style),
                ])));
                flat_idx += 1;

                if visible.is_empty() {
                    let hint_style = Style::default().fg(self.app.theme.text_muted);
                    let hint_text = "  └ no sessions yet";
                    items.push(ListItem::new(Line::from(vec![
                        Span::raw("    "),
                        Span::styled(hint_text, hint_style),
                    ])));
                }
            }

            for session in &visible {
                let is_sel = flat_idx == self.app.sidebar_selection;
                let is_cur = flat_idx == self.app.sidebar_cursor;
                let indicator = self.session_indicator(&session.id);
                let has_subagents = !self.app.subagent_sessions(i, &session.id).is_empty();
                let is_subagents_open =
                    self.app.subagents_expanded_for.as_deref() == Some(&session.id);
                let mut s_style = if is_sel {
                    Style::default()
                        .fg(self.app.theme.primary)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(self.app.theme.text_muted)
                };
                if is_cur {
                    s_style = self.with_cursor_bg(s_style);
                }
                let title = if session.title.is_empty() {
                    &session.id
                } else {
                    &session.title
                };
                let mut pad_style = Style::default();
                if is_cur {
                    pad_style = self.with_cursor_bg(pad_style);
                }
                let mut spans = vec![Span::styled("    ", pad_style)];
                spans.push(Span::styled("└ ", s_style));
                spans.push(self.indicator_span(&indicator, is_cur));
                if has_subagents {
                    let arrow = if is_subagents_open { "▼ " } else { "▶ " };
                    spans.push(Span::styled(arrow, s_style));
                }
                spans.push(Span::styled(title.to_string(), s_style));
                items.push(ListItem::new(Line::from(spans)));
                flat_idx += 1;

                if is_subagents_open {
                    let subagents = self.app.subagent_sessions(i, &session.id);
                    for sub in &subagents {
                        let sub_sel = flat_idx == self.app.sidebar_selection;
                        let sub_cur = flat_idx == self.app.sidebar_cursor;
                        let sub_indicator = self.session_indicator(&sub.id);
                        let mut sub_style = if sub_sel {
                            Style::default()
                                .fg(self.app.theme.primary)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(self.app.theme.text_muted)
                        };
                        if sub_cur {
                            sub_style = self.with_cursor_bg(sub_style);
                        }
                        let sub_title = if sub.title.is_empty() {
                            &sub.id
                        } else {
                            &sub.title
                        };
                        let mut sub_pad = Style::default();
                        if sub_cur {
                            sub_pad = self.with_cursor_bg(sub_pad);
                        }
                        let mut sub_spans = vec![Span::styled("      ", sub_pad)];
                        sub_spans.push(Span::styled("└ ", sub_style));
                        sub_spans.push(self.indicator_span(&sub_indicator, sub_cur));
                        sub_spans.push(Span::styled(sub_title.to_string(), sub_style));
                        items.push(ListItem::new(Line::from(sub_spans)));
                        flat_idx += 1;
                    }
                }
            }

            if self.app.has_more_sessions(i) {
                let is_sel = flat_idx == self.app.sidebar_selection;
                let is_cur = flat_idx == self.app.sidebar_cursor;
                let mut more_style = if is_sel {
                    Style::default()
                        .fg(self.app.theme.primary)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(self.app.theme.secondary)
                };
                if is_cur {
                    more_style = self.with_cursor_bg(more_style);
                }
                let mut pad_style = Style::default();
                if is_cur {
                    pad_style = self.with_cursor_bg(pad_style);
                }
                items.push(ListItem::new(Line::from(vec![
                    Span::styled("    ", pad_style),
                    Span::styled("└ more...", more_style),
                ])));
                flat_idx += 1;
            }
        }

        // "Add project" entry at the bottom
        let is_sel = flat_idx == self.app.sidebar_selection;
        let is_cur = flat_idx == self.app.sidebar_cursor;
        let mut add_style = if is_sel {
            Style::default()
                .fg(self.app.theme.primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.app.theme.primary)
        };
        if is_cur {
            add_style = self.with_cursor_bg(add_style);
        }
        items.push(ListItem::new(Line::from(vec![Span::styled(
            "  [+ Add Project]",
            add_style,
        )])));

        items
    }

    /// Compute the highest-priority indicator across all sessions in a project.
    fn project_indicator(&self, project_idx: usize) -> Indicator {
        let Some(project) = self.app.projects.get(project_idx) else {
            return Indicator::Idle;
        };
        let mut best = Indicator::Idle;
        for session in &project.sessions {
            let ind = self.session_indicator(&session.id);
            match ind {
                Indicator::Busy => return Indicator::Busy,
                Indicator::Input
                    if matches!(best, Indicator::Error | Indicator::Unseen | Indicator::Idle) =>
                {
                    best = Indicator::Input;
                }
                Indicator::Error if matches!(best, Indicator::Unseen | Indicator::Idle) => {
                    best = Indicator::Error;
                }
                Indicator::Unseen if matches!(best, Indicator::Idle) => {
                    best = Indicator::Unseen;
                }
                _ => {}
            }
        }
        best
    }
}
