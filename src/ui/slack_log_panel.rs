use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Clear, Widget};

use crate::slack::{SlackLogEntry, SlackLogLevel, SlackMetrics};
use crate::theme::ThemeColors;

/// Overlay panel that displays Slack event logs and metrics.
pub struct SlackLogPanel<'a> {
    theme: &'a ThemeColors,
    logs: &'a [SlackLogEntry],
    metrics: &'a SlackMetrics,
    scroll_offset: usize,
}

impl<'a> SlackLogPanel<'a> {
    pub fn new(
        theme: &'a ThemeColors,
        logs: &'a [SlackLogEntry],
        metrics: &'a SlackMetrics,
        scroll_offset: usize,
    ) -> Self {
        Self {
            theme,
            logs,
            metrics,
            scroll_offset,
        }
    }

    pub fn render_popup(&self, area: Rect, buf: &mut Buffer) {
        // 80% width, 70% height, centered
        let popup_width = (area.width * 80 / 100)
            .max(60)
            .min(area.width.saturating_sub(2));
        let popup_height = (area.height * 70 / 100)
            .max(14)
            .min(area.height.saturating_sub(2));

        let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(x, y, popup_width, popup_height);

        super::render_overlay_dim(area, buf);
        Clear.render(popup_area, buf);

        let block = Block::default().style(Style::default().bg(self.theme.background_panel));
        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        if inner.height < 6 || inner.width < 20 {
            return;
        }

        let cx = inner.x + 1;
        let cw = inner.width.saturating_sub(2);

        // ── Title row ──────────────────────────────────────────────────
        let title_y = inner.y;
        buf.set_string(
            cx,
            title_y,
            "Slack Logs",
            Style::default()
                .fg(self.theme.text)
                .add_modifier(Modifier::BOLD),
        );
        let esc_hint = "esc";
        let esc_x = cx + cw.saturating_sub(esc_hint.len() as u16);
        buf.set_string(
            esc_x,
            title_y,
            esc_hint,
            Style::default().fg(self.theme.text_muted),
        );

        // ── Metrics summary ────────────────────────────────────────────
        let m = self.metrics;
        let metrics_y = title_y + 1;
        let metrics_line = format!(
            "routed:{} fail:{} replies:{} batches:{} reconnect:{}",
            m.messages_routed, m.triage_failures, m.thread_replies,
            m.batches_sent, m.reconnections,
        );
        buf.set_string(
            cx,
            metrics_y,
            &metrics_line,
            Style::default().fg(self.theme.primary),
        );

        // ── Separator ──────────────────────────────────────────────────
        let sep_y = metrics_y + 1;
        let sep = "─".repeat(cw as usize);
        buf.set_string(
            cx,
            sep_y,
            &sep,
            Style::default().fg(self.theme.border_subtle),
        );

        // ── Log entries ────────────────────────────────────────────────
        let log_start_y = sep_y + 1;
        let max_rows = (inner.y + inner.height).saturating_sub(log_start_y + 1) as usize;

        if self.logs.is_empty() {
            buf.set_string(
                cx,
                log_start_y,
                "(no log entries yet)",
                Style::default().fg(self.theme.text_muted),
            );
        } else {
            // Show most recent entries at the bottom (newest last).
            let total = self.logs.len();
            let visible_count = max_rows.min(total);
            let start_idx = if total > max_rows + self.scroll_offset {
                total - max_rows - self.scroll_offset
            } else {
                0
            };
            let end_idx = (start_idx + visible_count).min(total);

            let mut cy = log_start_y;
            for entry in &self.logs[start_idx..end_idx] {
                if cy >= inner.y + inner.height - 1 {
                    break;
                }

                let level_str = match entry.level {
                    SlackLogLevel::Info => "INF",
                    SlackLogLevel::Warn => "WRN",
                    SlackLogLevel::Error => "ERR",
                };
                let level_color = match entry.level {
                    SlackLogLevel::Info => self.theme.success,
                    SlackLogLevel::Warn => self.theme.warning,
                    SlackLogLevel::Error => self.theme.error,
                };

                // Elapsed time since log entry was created.
                let elapsed = entry.timestamp.elapsed();
                let elapsed_str = if elapsed.as_secs() < 60 {
                    format!("{}s", elapsed.as_secs())
                } else if elapsed.as_secs() < 3600 {
                    format!("{}m", elapsed.as_secs() / 60)
                } else {
                    format!("{}h", elapsed.as_secs() / 3600)
                };

                // [INF 5s ago] message text...
                let prefix = format!("[{} {:>4} ago] ", level_str, elapsed_str);
                buf.set_string(cx, cy, &prefix, Style::default().fg(level_color));

                let msg_x = cx + prefix.len() as u16;
                let msg_width = cw.saturating_sub(prefix.len() as u16) as usize;
                let msg = if entry.message.len() > msg_width {
                    format!("{}…", &entry.message[..msg_width.saturating_sub(1)])
                } else {
                    entry.message.clone()
                };
                buf.set_string(
                    msg_x,
                    cy,
                    &msg,
                    Style::default().fg(self.theme.text),
                );

                cy += 1;
            }
        }

        // ── Bottom hint ────────────────────────────────────────────────
        let hint = "↑↓ scroll · Esc close";
        let hint_y = popup_area.y + popup_area.height - 1;
        buf.set_string(
            cx,
            hint_y,
            hint,
            Style::default().fg(self.theme.text_muted),
        );
    }
}
