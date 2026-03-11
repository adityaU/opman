use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::ui::sidebar::lerp_color;

use super::{TerminalPane, WatcherOverlayInfo};

impl<'a> TerminalPane<'a> {
    /// Render the watcher status bar overlay.
    pub(super) fn render_watcher_overlay(
        &self,
        area: Rect,
        buf: &mut Buffer,
        info: &WatcherOverlayInfo,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let theme = &self.app.theme;

        // Fill background
        let bg_style = Style::default()
            .bg(theme.background_panel)
            .fg(theme.text_muted);
        for x in area.x..area.right() {
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_style(bg_style);
                cell.set_char(' ');
            }
        }

        let mut spans = Vec::new();

        // Watcher label
        spans.push(Span::styled(
            " watcher ",
            Style::default()
                .fg(theme.background)
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));

        if info.is_busy || info.children_active {
            // Check for hang condition: busy with no activity past threshold
            let is_hung = !info.children_active
                && info
                    .hang_silent_secs
                    .map(|s| s >= info.hang_timeout_secs)
                    .unwrap_or(false);

            if is_hung {
                // Hang warning — pulsing red dot
                let dot_color =
                    lerp_color(theme.background_panel, theme.error, self.app.pulse_phase);
                spans.push(Span::styled(
                    "\u{25CF}",
                    Style::default().fg(dot_color).bg(theme.background_panel),
                ));
                spans.push(Span::styled(
                    " \u{26A0} running ",
                    Style::default()
                        .fg(theme.error)
                        .bg(theme.background_panel)
                        .add_modifier(Modifier::BOLD),
                ));

                let silent = info.hang_silent_secs.unwrap_or(0);
                let silent_display = if silent >= 60 {
                    format!("{}m{}s", silent / 60, silent % 60)
                } else {
                    format!("{}s", silent)
                };
                spans.push(Span::styled(
                    format!(
                        "\u{2014} no activity for {} (possibly hung)",
                        silent_display
                    ),
                    Style::default()
                        .fg(theme.text_muted)
                        .bg(theme.background_panel),
                ));
            } else {
                // Normal running state — pulsing dot
                let dot_color =
                    lerp_color(theme.background_panel, theme.warning, self.app.pulse_phase);
                spans.push(Span::styled(
                    "\u{25CF}",
                    Style::default().fg(dot_color).bg(theme.background_panel),
                ));

                if info.children_active {
                    spans.push(Span::styled(
                        " running ",
                        Style::default()
                            .fg(theme.warning)
                            .bg(theme.background_panel)
                            .add_modifier(Modifier::BOLD),
                    ));
                    spans.push(Span::styled(
                        "(subagent active)",
                        Style::default()
                            .fg(theme.text_muted)
                            .bg(theme.background_panel),
                    ));
                } else {
                    spans.push(Span::styled(
                        " running",
                        Style::default()
                            .fg(theme.warning)
                            .bg(theme.background_panel)
                            .add_modifier(Modifier::BOLD),
                    ));
                }
            }
        } else if let Some(elapsed) = info.idle_elapsed_secs {
            // Idle state with countdown
            let remaining = info.timeout_secs.saturating_sub(elapsed);

            // Pulsing dot (slower, calmer pulse for idle)
            let dot_color = lerp_color(theme.background_panel, theme.success, self.app.pulse_phase);
            spans.push(Span::styled(
                "\u{25CF}",
                Style::default().fg(dot_color).bg(theme.background_panel),
            ));
            spans.push(Span::styled(
                " idle ",
                Style::default()
                    .fg(theme.success)
                    .bg(theme.background_panel)
                    .add_modifier(Modifier::BOLD),
            ));

            if remaining > 0 {
                spans.push(Span::styled(
                    format!("{}s", elapsed),
                    Style::default()
                        .fg(theme.text)
                        .bg(theme.background_panel)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    format!(" / {}s", info.timeout_secs),
                    Style::default()
                        .fg(theme.text_muted)
                        .bg(theme.background_panel),
                ));
                spans.push(Span::styled(
                    " \u{2014} ",
                    Style::default()
                        .fg(theme.text_muted)
                        .bg(theme.background_panel),
                ));
                spans.push(Span::styled(
                    format!("continuing in {}s", remaining),
                    Style::default().fg(theme.accent).bg(theme.background_panel),
                ));
            } else {
                spans.push(Span::styled(
                    "sending continuation...",
                    Style::default()
                        .fg(theme.accent)
                        .bg(theme.background_panel)
                        .add_modifier(Modifier::BOLD),
                ));
            }
        } else {
            // Watcher configured but no idle data yet (just set up)
            spans.push(Span::styled(
                "\u{25CB} waiting",
                Style::default()
                    .fg(theme.text_muted)
                    .bg(theme.background_panel),
            ));
        }

        // Right-aligned timeout info
        let timeout_label = format!(" {}s timeout ", info.timeout_secs);
        let spans_width: usize = spans.iter().map(|s| s.content.len()).sum();
        let right_pad = (area.width as usize).saturating_sub(spans_width + timeout_label.len());
        if right_pad > 0 {
            spans.push(Span::styled(
                " ".repeat(right_pad),
                Style::default().bg(theme.background_panel),
            ));
        }
        spans.push(Span::styled(
            timeout_label,
            Style::default()
                .fg(theme.text_muted)
                .bg(theme.background_panel),
        ));

        let line = Line::from(spans);
        let para = Paragraph::new(line);
        Widget::render(para, area, buf);
    }
}
