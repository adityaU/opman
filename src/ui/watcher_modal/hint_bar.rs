use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::app::WatcherField;
use crate::theme::ThemeColors;

pub(super) fn render_hint_bar(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    state: &crate::app::WatcherModalState,
    theme: &ThemeColors,
) {
    let mut spans = vec![
        Span::styled("  Tab", Style::default().fg(theme.accent)),
        Span::styled(" next  ", Style::default().fg(theme.text_muted)),
        Span::styled("S-Tab", Style::default().fg(theme.accent)),
        Span::styled(" prev  ", Style::default().fg(theme.text_muted)),
    ];

    match state.active_field {
        WatcherField::SessionList => {
            spans.extend(vec![
                Span::styled("↑↓", Style::default().fg(theme.accent)),
                Span::styled(" navigate  ", Style::default().fg(theme.text_muted)),
                Span::styled("d", Style::default().fg(theme.accent)),
                Span::styled(" remove  ", Style::default().fg(theme.text_muted)),
            ]);
        }
        WatcherField::Message => {
            spans.extend(vec![
                Span::styled("Enter", Style::default().fg(theme.accent)),
                Span::styled(" newline  ", Style::default().fg(theme.text_muted)),
            ]);
        }
        WatcherField::IncludeOriginal => {
            spans.extend(vec![
                Span::styled("Space", Style::default().fg(theme.accent)),
                Span::styled(" toggle  ", Style::default().fg(theme.text_muted)),
            ]);
        }
        WatcherField::OriginalMessageList => {
            spans.extend(vec![
                Span::styled("↑↓", Style::default().fg(theme.accent)),
                Span::styled(" navigate  ", Style::default().fg(theme.text_muted)),
            ]);
        }
        WatcherField::TimeoutInput => {}
        WatcherField::HangMessage => {
            spans.extend(vec![
                Span::styled("Enter", Style::default().fg(theme.accent)),
                Span::styled(" newline  ", Style::default().fg(theme.text_muted)),
            ]);
        }
        WatcherField::HangTimeoutInput => {}
    }
    spans.extend(vec![
        Span::styled("Ctrl+D", Style::default().fg(theme.success)),
        Span::styled(" submit  ", Style::default().fg(theme.text_muted)),
        Span::styled("Esc", Style::default().fg(theme.warning)),
        Span::styled(" close", Style::default().fg(theme.text_muted)),
    ]);

    let line = Line::from(spans);
    Paragraph::new(line).render(Rect::new(x, y, width, 1), buf);
}
