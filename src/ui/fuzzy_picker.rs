use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config, Injector, Nucleo, Utf32String};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Widget};

use crate::app::App;
use crate::theme::ThemeColors;

/// State for the fuzzy directory picker (lives in App).
pub struct FuzzyPickerState {
    pub matcher: Nucleo<String>,
    pub query: String,
    pub prev_query: String,
    pub cursor_pos: usize,
    pub selected: u32,
    pub scroll_offset: u32,
    walk_complete_flag: Arc<AtomicBool>,
    /// Sidebar projects shown by default when query is empty.
    /// Each entry is (display_name, raw_path).
    pub existing_projects: Vec<(String, String)>,
}

impl std::fmt::Debug for FuzzyPickerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FuzzyPickerState")
            .field("query", &self.query)
            .field("selected", &self.selected)
            .field("walk_complete", &self.walk_complete())
            .finish()
    }
}

impl FuzzyPickerState {
    /// Create a new fuzzy picker and start scanning directories under `root`.
    #[allow(dead_code)]
    pub fn new(root: PathBuf) -> Self {
        Self::new_with_existing(root, Vec::new())
    }

    /// Create a fuzzy picker that includes existing project paths in results.
    pub fn new_with_existing(root: PathBuf, existing_projects: Vec<String>) -> Self {
        let matcher = Nucleo::new(
            Config::DEFAULT.match_paths(),
            Arc::new(|| {}),
            None, // use default thread count
            1,    // single match column
        );

        let injector = matcher.injector();
        let walk_done = Arc::new(AtomicBool::new(false));
        let walk_done_clone = Arc::clone(&walk_done);

        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let home_str = home.to_string_lossy().to_string();
        let sidebar_projects: Vec<(String, String)> = existing_projects
            .iter()
            .map(|p| {
                let display = if p.starts_with(&home_str) {
                    format!("~{} ★", &p[home_str.len()..])
                } else {
                    format!("{} ★", p)
                };
                (display, p.clone())
            })
            .collect();

        std::thread::spawn(move || {
            walk_directories(root, injector, existing_projects);
            walk_done_clone.store(true, Ordering::Release);
        });

        Self {
            matcher,
            query: String::new(),
            prev_query: String::new(),
            cursor_pos: 0,
            selected: 0,
            scroll_offset: 0,
            walk_complete_flag: walk_done,
            existing_projects: sidebar_projects,
        }
    }

    pub fn walk_complete(&self) -> bool {
        self.walk_complete_flag.load(Ordering::Acquire)
    }

    /// Tick the matcher and update pattern if query changed.
    /// Returns true if results changed.
    pub fn tick(&mut self) -> bool {
        // Update pattern if query changed
        if self.query != self.prev_query {
            let append = self.query.starts_with(&self.prev_query) && !self.prev_query.is_empty();
            self.matcher.pattern.reparse(
                0,
                &self.query,
                CaseMatching::Smart,
                Normalization::Smart,
                append,
            );
            self.prev_query = self.query.clone();
        }

        let status = self.matcher.tick(10);
        status.changed
    }

    /// Get the total number of matched items.
    pub fn matched_count(&self) -> u32 {
        self.matcher.snapshot().matched_item_count()
    }

    /// Get the total number of items (matched + unmatched).
    pub fn total_count(&self) -> u32 {
        self.matcher.snapshot().item_count()
    }

    /// Get the currently selected path, if any.
    pub fn selected_path(&self) -> Option<String> {
        if self.query.is_empty() && !self.existing_projects.is_empty() {
            let count = self.existing_projects.len() as u32;
            let idx = self.selected.min(count.saturating_sub(1));
            return self
                .existing_projects
                .get(idx as usize)
                .map(|(_, p)| p.clone());
        }

        let snapshot = self.matcher.snapshot();
        let count = snapshot.matched_item_count();
        if count == 0 {
            return None;
        }
        // Selection is from the bottom (fzf-style: 0 = bottom of list)
        let idx = self.selected.min(count.saturating_sub(1));
        let items: Vec<_> = snapshot.matched_items(0..count).collect();
        // Bottom-up: selected=0 means the top-scored item (first in matched_items)
        items.get(idx as usize).map(|item| item.data.clone())
    }

    /// Move selection up (towards less relevant items).
    pub fn move_up(&mut self) {
        let count = self.visible_count();
        if count > 0 && self.selected < count.saturating_sub(1) {
            self.selected += 1;
        }
    }

    /// Move selection down (towards more relevant items).
    pub fn move_down(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    fn visible_count(&self) -> u32 {
        if self.query.is_empty() && !self.existing_projects.is_empty() {
            self.existing_projects.len() as u32
        } else {
            self.matched_count()
        }
    }

    /// Insert a character at cursor position.
    pub fn insert_char(&mut self, c: char) {
        self.query.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Delete character before cursor.
    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.query[..self.cursor_pos]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.query.remove(prev);
            self.cursor_pos = prev;
            self.selected = 0;
            self.scroll_offset = 0;
        }
    }

    /// Move cursor left.
    pub fn cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos = self.query[..self.cursor_pos]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    /// Move cursor right.
    pub fn cursor_right(&mut self) {
        if self.cursor_pos < self.query.len() {
            self.cursor_pos = self.query[self.cursor_pos..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor_pos + i)
                .unwrap_or(self.query.len());
        }
    }
}

fn walk_directories(root: PathBuf, injector: Injector<String>, existing_projects: Vec<String>) {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let home_str = home.to_string_lossy().to_string();

    let mut seen = std::collections::HashSet::new();

    // Inject existing projects first so they appear immediately
    for proj_path in &existing_projects {
        seen.insert(proj_path.clone());
        let display = if proj_path.starts_with(&home_str) {
            format!("~{} ★", &proj_path[home_str.len()..])
        } else {
            format!("{} ★", proj_path)
        };
        let data = proj_path.clone();
        let _ = injector.push(data, |_, cols| {
            cols[0] = Utf32String::from(display.as_str());
        });
    }

    let walker = ignore::WalkBuilder::new(&root)
        .standard_filters(false)
        .follow_links(true)
        .max_depth(Some(5))
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            if entry.depth() > 0 && name.starts_with('.') {
                return false;
            }
            match name.as_ref() {
                "node_modules" | "target" | "__pycache__" | ".git" | "vendor" | "dist"
                | "build" | ".cache" | "Library" | "Pictures" | "Music" | "Movies" => false,
                _ => true,
            }
        })
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            continue;
        }

        if entry.depth() == 0 {
            continue;
        }

        let path = entry.path().to_string_lossy().to_string();

        // Skip paths already injected as existing projects
        if seen.contains(&path) {
            continue;
        }

        let display = if path.starts_with(&home_str) {
            format!("~{}", &path[home_str.len()..])
        } else {
            path.clone()
        };

        let _ = injector.push(path, |_, cols| {
            cols[0] = Utf32String::from(display.as_str());
        });
    }
}

/// Render-only widget for the fuzzy picker overlay.
pub struct FuzzyPicker<'a> {
    app: &'a App,
}

impl<'a> FuzzyPicker<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app }
    }

    pub fn render_popup(self, area: Rect, buf: &mut Buffer) {
        let state = match &self.app.fuzzy_picker {
            Some(s) => s,
            None => return,
        };

        let theme = &self.app.theme;

        let popup_width = 80u16.min(area.width.saturating_sub(2));
        let max_list_h = area.height / 2;
        let popup_height = (max_list_h + 6).max(10).min(area.height.saturating_sub(2));
        let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width,
            height: popup_height.min(area.height.saturating_sub(popup_y.saturating_sub(area.y))),
        };

        super::render_overlay_dim(area, buf);
        Clear.render(popup_area, buf);

        let block = Block::default().style(Style::default().bg(theme.background_panel));
        let panel_inner = block.inner(popup_area);
        block.render(popup_area, buf);

        let inner = Rect {
            x: panel_inner.x + 2,
            y: panel_inner.y + 1,
            width: panel_inner.width.saturating_sub(4),
            height: panel_inner.height.saturating_sub(1),
        };

        if inner.height < 5 {
            return;
        }

        let title_span = Span::styled(
            "Search",
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        );
        let esc_span = Span::styled("esc", Style::default().fg(theme.text_muted));
        let title_line = Line::from(vec![
            title_span,
            Span::raw(" ".repeat((inner.width as usize).saturating_sub(6 + 3))),
            esc_span,
        ]);
        buf.set_line(inner.x, inner.y, &title_line, inner.width);

        let content_y = inner.y + 1;
        let content_height = inner.height.saturating_sub(1);

        if content_height < 3 {
            return;
        }

        let input_y = content_y + content_height.saturating_sub(1);
        let hint_y = content_y + content_height.saturating_sub(2);
        let separator_y = content_y + content_height.saturating_sub(3);
        let results_height = content_height.saturating_sub(3);

        render_input_line(buf, inner.x, input_y, inner.width, state, theme);
        render_hint_line(buf, inner.x, hint_y, inner.width, state, theme);
        render_separator(buf, inner.x, separator_y, inner.width, theme);

        if results_height > 0 {
            let results_area = Rect {
                x: inner.x,
                y: content_y,
                width: inner.width,
                height: results_height,
            };
            render_results(buf, results_area, state, theme);
        }
    }
}

fn render_input_line(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    state: &FuzzyPickerState,
    theme: &ThemeColors,
) {
    // Prompt indicator
    let prompt = "> ";
    buf.set_string(
        x,
        y,
        prompt,
        Style::default()
            .fg(theme.primary)
            .add_modifier(Modifier::BOLD),
    );

    // Query text
    let query_x = x + prompt.len() as u16;
    buf.set_string(query_x, y, &state.query, Style::default().fg(theme.text));

    // Cursor
    let cursor_x = query_x + state.cursor_pos as u16;
    if cursor_x < x + width {
        let cursor_char = state.query[state.cursor_pos..]
            .chars()
            .next()
            .unwrap_or(' ');
        buf.set_string(
            cursor_x,
            y,
            cursor_char.to_string(),
            Style::default()
                .fg(theme.background)
                .bg(theme.text)
                .add_modifier(Modifier::BOLD),
        );
    }
}

fn render_hint_line(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    state: &FuzzyPickerState,
    theme: &ThemeColors,
) {
    let (count_text, status_indicator) =
        if state.query.is_empty() && !state.existing_projects.is_empty() {
            let n = state.existing_projects.len();
            (
                format!("  {} project{}", n, if n == 1 { "" } else { "s" }),
                "",
            )
        } else {
            let matched = state.matched_count();
            let total = state.total_count();
            let indicator = if state.walk_complete() {
                ""
            } else {
                " (scanning...)"
            };
            (format!("  {}/{}", matched, total), indicator)
        };

    let spans = vec![
        Span::styled(
            format!("{}{}", count_text, status_indicator),
            Style::default().fg(theme.text_muted),
        ),
        Span::raw("  "),
        Span::styled("enter", Style::default().fg(theme.accent)),
        Span::styled(": select  ", Style::default().fg(theme.text_muted)),
        Span::styled("esc", Style::default().fg(theme.accent)),
        Span::styled(": cancel  ", Style::default().fg(theme.text_muted)),
        Span::styled("up/down", Style::default().fg(theme.accent)),
        Span::styled(": navigate", Style::default().fg(theme.text_muted)),
    ];

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
    let hint_area = Rect {
        x,
        y,
        width,
        height: 1,
    };
    paragraph.render(hint_area, buf);
}

fn render_separator(buf: &mut Buffer, x: u16, y: u16, width: u16, theme: &ThemeColors) {
    let sep: String = "─".repeat(width as usize);
    buf.set_string(x, y, &sep, Style::default().fg(theme.border_subtle));
}

fn render_results(buf: &mut Buffer, area: Rect, state: &FuzzyPickerState, theme: &ThemeColors) {
    if state.query.is_empty() && !state.existing_projects.is_empty() {
        render_existing_projects(buf, area, state, theme);
        return;
    }

    let snapshot = state.matcher.snapshot();
    let matched = snapshot.matched_item_count();

    if matched == 0 {
        // Show "no results" message
        if !state.query.is_empty() {
            buf.set_string(
                area.x + 2,
                area.y + area.height / 2,
                "No matching directories",
                Style::default().fg(theme.text_muted),
            );
        }
        return;
    }

    let visible_count = area.height as u32;
    let selected = state.selected.min(matched.saturating_sub(1));

    // Ensure selected item is visible (scroll adjustment)
    let scroll_offset = if selected >= state.scroll_offset + visible_count {
        selected - visible_count + 1
    } else if selected < state.scroll_offset {
        selected
    } else {
        state.scroll_offset
    };

    // Render items bottom-up (fzf style: most relevant at bottom, near input)
    let items: Vec<_> = snapshot
        .matched_items(scroll_offset..matched.min(scroll_offset + visible_count))
        .collect();

    for (i, item) in items.iter().enumerate() {
        let row = area.y + area.height.saturating_sub(1) - i as u16;
        if row < area.y {
            break;
        }

        let display = item.matcher_columns[0].to_string();
        let is_selected = (scroll_offset + i as u32) == selected;

        let style = if is_selected {
            Style::default()
                .fg(theme.background)
                .bg(theme.primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text)
        };

        // Clear the row
        let blank: String = " ".repeat(area.width as usize);
        buf.set_string(area.x, row, &blank, style);

        // Render path with indicator
        let indicator = if is_selected { "> " } else { "  " };
        buf.set_string(area.x, row, indicator, style);

        let max_path_width = (area.width as usize).saturating_sub(2);
        let truncated = if display.len() > max_path_width {
            format!("...{}", &display[display.len() - max_path_width + 3..])
        } else {
            display
        };
        buf.set_string(area.x + 2, row, &truncated, style);
    }
}

fn render_existing_projects(
    buf: &mut Buffer,
    area: Rect,
    state: &FuzzyPickerState,
    theme: &ThemeColors,
) {
    let count = state.existing_projects.len() as u32;
    if count == 0 {
        return;
    }

    let visible_count = area.height as u32;
    let selected = state.selected.min(count.saturating_sub(1));

    let scroll_offset = if selected >= state.scroll_offset + visible_count {
        selected - visible_count + 1
    } else if selected < state.scroll_offset {
        selected
    } else {
        state.scroll_offset
    };

    let end = count.min(scroll_offset + visible_count);

    for i in scroll_offset..end {
        let row_idx = i - scroll_offset;
        let row = area.y + area.height.saturating_sub(1) - row_idx as u16;
        if row < area.y {
            break;
        }

        let (ref display, _) = state.existing_projects[i as usize];
        let is_selected = i == selected;

        let style = if is_selected {
            Style::default()
                .fg(theme.background)
                .bg(theme.primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text)
        };

        // Clear the row
        let blank: String = " ".repeat(area.width as usize);
        buf.set_string(area.x, row, &blank, style);

        // Render path with indicator
        let indicator = if is_selected { "> " } else { "  " };
        buf.set_string(area.x, row, indicator, style);

        let max_path_width = (area.width as usize).saturating_sub(2);
        let truncated = if display.len() > max_path_width {
            format!("...{}", &display[display.len() - max_path_width + 3..])
        } else {
            display.clone()
        };
        buf.set_string(area.x + 2, row, &truncated, style);
    }
}
