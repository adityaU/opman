use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;

use super::lerp_color;
use super::Sidebar;

impl<'a> Sidebar<'a> {
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

            let has_active = project.sessions.iter().any(|s| {
                if self.app.active_sessions.contains(&s.id) {
                    return true;
                }
                self.app
                    .subagent_sessions(i, &s.id)
                    .iter()
                    .any(|sub| self.app.active_sessions.contains(&sub.id))
            });

            let mut spans = vec![Span::styled(marker, style)];
            if has_active {
                let dot_color = lerp_color(
                    self.app.theme.background,
                    self.app.theme.accent,
                    self.app.pulse_phase,
                );
                let mut dot_style = Style::default().fg(dot_color);
                if is_cursor {
                    dot_style = self.with_cursor_bg(dot_style);
                }
                spans.push(Span::styled("● ", dot_style));
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
                let is_active = self.app.active_sessions.contains(&session.id);
                let has_active_subagent = self
                    .app
                    .subagent_sessions(i, &session.id)
                    .iter()
                    .any(|sub| self.app.active_sessions.contains(&sub.id));
                let show_dot = is_active || has_active_subagent;
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
                if show_dot {
                    let dot_color = lerp_color(
                        self.app.theme.background,
                        self.app.theme.accent,
                        self.app.pulse_phase,
                    );
                    let mut dot_style = Style::default().fg(dot_color);
                    if is_cur {
                        dot_style = self.with_cursor_bg(dot_style);
                    }
                    spans.push(Span::styled("● ", dot_style));
                }
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
                        let sub_active = self.app.active_sessions.contains(&sub.id);
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
                        if sub_active {
                            let dot_color = lerp_color(
                                self.app.theme.background,
                                self.app.theme.accent,
                                self.app.pulse_phase,
                            );
                            let mut dot_style = Style::default().fg(dot_color);
                            if sub_cur {
                                dot_style = self.with_cursor_bg(dot_style);
                            }
                            sub_spans.push(Span::styled("● ", dot_style));
                        }
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
}
