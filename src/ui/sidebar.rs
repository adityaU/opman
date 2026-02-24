use std::cmp::min;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, Widget};

use ratatui::style::Color;

use crate::app::App;

pub(crate) fn lerp_color(from: Color, to: Color, t: f64) -> Color {
    let (r1, g1, b1) = match from {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (0, 0, 0),
    };
    let (r2, g2, b2) = match to {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (255, 255, 255),
    };
    Color::Rgb(
        (r1 as f64 + (r2 as f64 - r1 as f64) * t) as u8,
        (g1 as f64 + (g2 as f64 - g1 as f64) * t) as u8,
        (b1 as f64 + (b2 as f64 - b1 as f64) * t) as u8,
    )
}

/// Sidebar widget that displays the project list with sessions,
/// and an optional keyboard shortcuts help panel at the bottom.
pub struct Sidebar<'a> {
    app: &'a App,
}

impl<'a> Sidebar<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app }
    }

    /// Apply cursor (j/k highlight) background to an existing style.
    fn with_cursor_bg(&self, style: Style) -> Style {
        style.bg(self.app.theme.background_element)
    }

    /// Build the list items for the project list.
    ///
    /// Two independent highlights:
    ///   - **cursor** (`sidebar_cursor`): background highlight via `background_element`
    ///   - **selected** (`sidebar_selection`): bold + primary foreground (active session)
    fn build_items(&self) -> Vec<ListItem<'a>> {
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

impl<'a> Widget for Sidebar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut items = self.build_items();

        if let Some(idx) = self.app.confirm_delete {
            let name = self
                .app
                .projects
                .get(idx)
                .map(|p| p.name.as_str())
                .unwrap_or("?");
            items.push(ListItem::new(Line::from("")));
            items.push(ListItem::new(Line::from(Span::styled(
                format!("  Delete \"{}\"? (y/N)", name),
                Style::default().fg(self.app.theme.warning),
            ))));
        } else {
            items.push(ListItem::new(Line::from("")));
            items.push(ListItem::new(Line::from(Span::styled(
                "  ? for shortcuts",
                Style::default().fg(self.app.theme.text_muted),
            ))));
        }

        // Scrolling follows the cursor (j/k position)
        let max_visible = area.height.saturating_sub(1) as usize;
        let total_items = items.len();
        let sidebar_cursor = self.app.sidebar_cursor;

        let scroll_offset = if sidebar_cursor >= max_visible {
            sidebar_cursor - max_visible + 1
        } else {
            0
        };

        let end = min(scroll_offset + max_visible, total_items);
        let visible_items: Vec<ListItem> = items
            .into_iter()
            .skip(scroll_offset)
            .take(end - scroll_offset)
            .collect();

        let list = List::new(visible_items);
        Widget::render(list, area, buf);
    }
}

const MAX_VISIBLE_SEARCH_RESULTS: usize = 20;

pub struct SessionSearchPanel<'a> {
    app: &'a App,
}

impl<'a> SessionSearchPanel<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app }
    }

    pub fn render_popup(&self, area: Rect, buf: &mut Buffer) {
        let popup_width = ((area.width as f32) * 0.6) as u16;
        let popup_height = ((area.height as f32) * 0.5) as u16;
        let x = (area.width.saturating_sub(popup_width)) / 2;
        let y = (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(x, y, popup_width, popup_height);

        Clear.render(popup_area, buf);

        let block = Block::default().style(Style::default().bg(self.app.theme.background_panel));
        Widget::render(block, popup_area, buf);

        let title_area = Rect::new(
            popup_area.x + 1,
            popup_area.y,
            popup_area.width.saturating_sub(2),
            1,
        );
        buf.set_string(
            title_area.x,
            title_area.y,
            "Search Sessions",
            Style::default()
                .fg(self.app.theme.text)
                .add_modifier(Modifier::BOLD),
        );

        let input_area = Rect::new(
            popup_area.x + 1,
            popup_area.y + 1,
            popup_area.width.saturating_sub(2),
            1,
        );
        let input_text = format!("> {}", self.app.session_search_buffer);
        buf.set_string(
            input_area.x,
            input_area.y,
            &input_text,
            Style::default().fg(self.app.theme.text),
        );

        let cursor_x = input_area.x + 2 + self.app.session_search_cursor as u16;
        if cursor_x < input_area.x + input_area.width {
            buf.set_string(
                cursor_x,
                input_area.y,
                " ",
                Style::default()
                    .bg(self.app.theme.text)
                    .fg(self.app.theme.background),
            );
        }

        let sep_area = Rect::new(
            popup_area.x + 1,
            popup_area.y + 2,
            popup_area.width.saturating_sub(2),
            1,
        );
        let sep = "\u{2500}".repeat(sep_area.width as usize);
        buf.set_string(
            sep_area.x,
            sep_area.y,
            &sep,
            Style::default().fg(self.app.theme.border_subtle),
        );

        let list_y_start = popup_area.y + 3;
        let max_visible = min(
            (popup_area.height as usize).saturating_sub(4),
            MAX_VISIBLE_SEARCH_RESULTS,
        );

        let selected = self.app.session_search_selected;
        let total = self.app.session_search_results.len();
        let scroll_offset = if selected >= max_visible {
            selected - max_visible + 1
        } else {
            0
        };
        let end = min(scroll_offset + max_visible, total);

        for (i, idx) in (scroll_offset..end).enumerate() {
            let row_y = list_y_start + i as u16;
            if row_y >= popup_area.y + popup_area.height.saturating_sub(1) {
                break;
            }

            let session = &self.app.session_search_results[idx];
            let is_selected = idx == selected;

            let style = if is_selected {
                Style::default()
                    .bg(self.app.theme.primary)
                    .fg(self.app.theme.background)
            } else {
                Style::default().fg(self.app.theme.text)
            };

            let title = if session.title.is_empty() {
                &session.id
            } else {
                &session.title
            };
            let max_title_len = (popup_area.width.saturating_sub(4)) as usize;
            let truncated = if title.len() > max_title_len {
                &title[..max_title_len.saturating_sub(3)]
            } else {
                title
            };

            let display = format!("  {}", truncated);
            buf.set_string(popup_area.x + 1, row_y, &display, style);

            if is_selected {
                let remaining = (popup_area.width as usize).saturating_sub(display.len() + 1);
                if remaining > 0 {
                    buf.set_string(
                        popup_area.x + 1 + display.len() as u16,
                        row_y,
                        &" ".repeat(remaining),
                        style,
                    );
                }
            }
        }

        if total > max_visible {
            let indicator = format!(" [{}/{}]", selected + 1, total);
            let indicator_x =
                popup_area.x + popup_area.width.saturating_sub(indicator.len() as u16 + 1);
            let indicator_y = popup_area.y + popup_area.height.saturating_sub(2);
            buf.set_string(
                indicator_x,
                indicator_y,
                &indicator,
                Style::default().fg(self.app.theme.text_muted),
            );
        }

        let hint_y = popup_area.y + popup_area.height.saturating_sub(1);
        buf.set_string(
            popup_area.x + 1,
            hint_y,
            "\u{2191}\u{2193} navigate  Enter select  Esc cancel",
            Style::default().fg(self.app.theme.text_muted),
        );
    }
}
