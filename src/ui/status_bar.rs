use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::app::{App, ServerStatus};
use crate::ui::layout_manager::PanelId;
use crate::ui::sidebar::lerp_color;
use crate::vim_mode::VimMode;

/// Status bar widget displayed at the bottom of the screen.
///
/// Shows: current project name, git branch, server status, focus mode.
pub struct StatusBar<'a> {
    app: &'a App,
}

impl<'a> StatusBar<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app }
    }
}

impl<'a> Widget for StatusBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut spans = Vec::new();

        let mode_style = match self.app.vim_mode {
            VimMode::Normal => Style::default()
                .fg(self.app.theme.background)
                .bg(self.app.theme.accent)
                .add_modifier(Modifier::BOLD),
            VimMode::Insert => Style::default()
                .fg(self.app.theme.background)
                .bg(self.app.theme.success)
                .add_modifier(Modifier::BOLD),
            VimMode::Command => Style::default()
                .fg(self.app.theme.background)
                .bg(self.app.theme.warning)
                .add_modifier(Modifier::BOLD),
            VimMode::WhichKey => Style::default()
                .fg(self.app.theme.background)
                .bg(self.app.theme.info)
                .add_modifier(Modifier::BOLD),
            VimMode::Resize => Style::default()
                .fg(self.app.theme.background)
                .bg(self.app.theme.warning)
                .add_modifier(Modifier::BOLD),
        };
        spans.push(Span::styled(
            format!(" {} ", self.app.vim_mode.label()),
            mode_style,
        ));

        // Project name
        if let Some(project) = self.app.active_project() {
            spans.push(Span::styled(
                format!(" {} ", project.name),
                Style::default()
                    .fg(self.app.theme.background)
                    .bg(self.app.theme.primary)
                    .add_modifier(Modifier::BOLD),
            ));

            // Git branch
            if !project.git_branch.is_empty() {
                spans.push(Span::styled(
                    format!("  {} ", project.git_branch),
                    Style::default().fg(self.app.theme.accent),
                ));
            }

            // Server status
            let status = self.app.project_server_status(self.app.active_project);
            match status {
                ServerStatus::Running => {
                    let dot_color = lerp_color(
                        self.app.theme.background,
                        self.app.theme.accent,
                        self.app.pulse_phase,
                    );
                    spans.push(Span::raw("  "));
                    spans.push(Span::styled("●", Style::default().fg(dot_color)));
                    spans.push(Span::styled(
                        " running ",
                        Style::default().fg(self.app.theme.success),
                    ));

                    // Track URL x-range for click-to-copy
                    let url_text = crate::app::base_url();
                    let start_x: u16 = area.x + spans.iter().map(|s| s.width() as u16).sum::<u16>();
                    let end_x = start_x + url_text.len() as u16;
                    self.app.status_bar_url_range.set(Some((start_x, end_x)));

                    spans.push(Span::styled(
                        url_text,
                        Style::default().fg(self.app.theme.text_muted),
                    ));
                }
                _ => {
                    let (status_text, status_color) = match status {
                        ServerStatus::Running => unreachable!(),
                        ServerStatus::Starting => ("◐ starting", self.app.theme.warning),
                        ServerStatus::Stopped => ("○ stopped", self.app.theme.error),
                        ServerStatus::Error => ("✕ error", self.app.theme.error),
                    };
                    spans.push(Span::raw("  "));
                    spans.push(Span::styled(status_text, Style::default().fg(status_color)));
                    self.app.status_bar_url_range.set(None);
                }
            }

            // Token usage and context window — always shown when stats exist
            if let Some(session_id) = &project.active_session {
                if let Some(stats) = self.app.session_stats.get(session_id) {
                    let total_tokens = stats.total_tokens();

                    let token_display = if total_tokens >= 1_000_000 {
                        format!("{:.1}M", total_tokens as f64 / 1_000_000.0)
                    } else if total_tokens >= 1_000 {
                        format!("{:.1}K", total_tokens as f64 / 1_000.0)
                    } else {
                        format!("{}", total_tokens)
                    };

                    let context_window = self
                        .app
                        .model_limits
                        .get(&self.app.active_project)
                        .map(|ml| ml.context_window)
                        .unwrap_or(0);

                    spans.push(Span::raw("  "));

                    if context_window > 0 {
                        let pct =
                            ((total_tokens as f64 / context_window as f64) * 100.0).round() as u64;
                        let pct_color = if pct >= 90 {
                            self.app.theme.error
                        } else if pct >= 70 {
                            self.app.theme.warning
                        } else {
                            self.app.theme.text_muted
                        };
                        spans.push(Span::styled(
                            format!(" {}  {}%", token_display, pct),
                            Style::default().fg(pct_color),
                        ));
                    } else {
                        spans.push(Span::styled(
                            format!(" {}", token_display),
                            Style::default().fg(self.app.theme.text_muted),
                        ));
                    }

                    // Direction arrow: compare current total with previous total
                    let prev = stats.prev_total_tokens;
                    if total_tokens > prev && prev > 0 {
                        spans.push(Span::styled(
                            " ▲",
                            Style::default().fg(self.app.theme.error),
                        ));
                    } else if total_tokens < prev {
                        spans.push(Span::styled(
                            " ▼",
                            Style::default().fg(self.app.theme.success),
                        ));
                    }

                    if stats.cost > 0.0 {
                        spans.push(Span::styled(
                            format!("  ${:.4}", stats.cost),
                            Style::default().fg(self.app.theme.text_muted),
                        ));
                    }
                }
            }
        } else {
            spans.push(Span::styled(
                " No project selected ",
                Style::default().fg(self.app.theme.text_muted),
            ));
        }

        if let Some((ref msg, ts)) = self.app.toast_message {
            if ts.elapsed() < std::time::Duration::from_secs(2) {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    msg,
                    Style::default()
                        .fg(self.app.theme.accent)
                        .add_modifier(Modifier::BOLD),
                ));
            }
        }

        let panel_label = match self.app.layout.focused {
            PanelId::Sidebar => "SIDEBAR",
            PanelId::TerminalPane => "OPENCODE",
            PanelId::NeovimPane => "NEOVIM",
            PanelId::IntegratedTerminal => "TERMINAL",
            PanelId::GitPanel => "GIT",
        };
        let panel_span_text = format!(" {} ", panel_label);
        let panel_style = Style::default()
            .fg(self.app.theme.background)
            .bg(self.app.theme.accent)
            .add_modifier(Modifier::BOLD);

        let left_width: usize = spans.iter().map(|s| s.width()).sum();
        let right_width = panel_span_text.len();
        let total_width = area.width as usize;
        let padding = total_width.saturating_sub(left_width + right_width);

        if padding > 0 {
            spans.push(Span::styled(
                " ".repeat(padding),
                Style::default().bg(self.app.theme.background_panel),
            ));
        }
        spans.push(Span::styled(panel_span_text, panel_style));

        let line = Line::from(spans);
        let paragraph =
            Paragraph::new(line).style(Style::default().bg(self.app.theme.background_panel));

        Widget::render(paragraph, area, buf);
    }
}
