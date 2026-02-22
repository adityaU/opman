use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Clear, Widget};

use crate::theme::ThemeColors;
use crate::which_key::{generate_cheatsheet_sections, RuntimeKeyBinding};

pub struct CheatSheet<'a> {
    theme: &'a ThemeColors,
    keymap: &'a [RuntimeKeyBinding],
}

impl<'a> CheatSheet<'a> {
    pub fn new(theme: &'a ThemeColors, keymap: &'a [RuntimeKeyBinding]) -> Self {
        Self { theme, keymap }
    }

    pub fn render_popup(&self, area: Rect, buf: &mut Buffer) {
        let sections = generate_cheatsheet_sections(self.keymap);

        let max_key_width: u16 = sections
            .iter()
            .flat_map(|(_, items)| items.iter().map(|(k, _)| k.len() as u16))
            .max()
            .unwrap_or(0);

        let max_desc_width: u16 = sections
            .iter()
            .flat_map(|(_, items)| items.iter().map(|(_, d)| d.len() as u16))
            .max()
            .unwrap_or(0);

        let col_content_width = max_key_width + 3 + max_desc_width;

        let section_heights: Vec<u16> = sections
            .iter()
            .enumerate()
            .map(|(i, (_, items))| {
                let sep = if i > 0 { 1u16 } else { 0 };
                sep + 1 + items.len() as u16
            })
            .collect();

        let total_content_height: u16 = section_heights.iter().sum();
        let avail_height = area.height.saturating_sub(6);

        // Determine columns needed via greedy bin-packing
        let num_cols = if total_content_height <= avail_height {
            1u16
        } else {
            let mut cols = 2u16;
            while cols < 6 {
                let target_h = (total_content_height + cols - 1) / cols;
                if target_h <= avail_height {
                    break;
                }
                cols += 1;
            }
            cols
        };

        let target_col_height = if num_cols > 1 {
            (total_content_height + num_cols - 1) / num_cols
        } else {
            total_content_height
        };

        let mut columns: Vec<Vec<usize>> = vec![vec![]];
        let mut current_col_h: u16 = 0;

        for (si, &sh) in section_heights.iter().enumerate() {
            if current_col_h > 0
                && current_col_h + sh > target_col_height + 2
                && columns.len() < num_cols as usize
            {
                columns.push(vec![]);
                current_col_h = 0;
            }
            columns.last_mut().unwrap().push(si);
            current_col_h += sh;
        }

        let actual_cols = columns.len() as u16;
        let col_gap = 3u16;
        let popup_inner_w = if actual_cols == 1 {
            col_content_width
        } else {
            actual_cols * col_content_width + (actual_cols - 1) * col_gap
        };
        let popup_width = (popup_inner_w + 4).min(area.width.saturating_sub(4));

        let max_col_height: u16 = columns
            .iter()
            .map(|col_secs| col_secs.iter().map(|&si| section_heights[si]).sum::<u16>())
            .max()
            .unwrap_or(0);
        let popup_height = (max_col_height + 4).min(area.height.saturating_sub(2));

        let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(
            x,
            y,
            popup_width,
            popup_height.min(area.height.saturating_sub(y.saturating_sub(area.y))),
        );

        super::render_overlay_dim(area, buf);
        Clear.render(popup_area, buf);

        let block = Block::default().style(Style::default().bg(self.theme.background_panel));
        let panel_inner = block.inner(popup_area);
        Widget::render(block, popup_area, buf);

        let inner_x = panel_inner.x + 2;
        let inner_w = panel_inner.width.saturating_sub(4);
        let title_y = panel_inner.y + 1;

        buf.set_string(
            inner_x,
            title_y,
            "Keybindings",
            Style::default()
                .fg(self.theme.text)
                .add_modifier(Modifier::BOLD),
        );
        let esc_hint = "esc";
        let esc_x = inner_x + inner_w.saturating_sub(esc_hint.len() as u16);
        buf.set_string(
            esc_x,
            title_y,
            esc_hint,
            Style::default().fg(self.theme.text_muted),
        );

        let content_y = title_y + 2;
        let max_y = panel_inner.y + panel_inner.height;

        for (col_idx, col_sections) in columns.iter().enumerate() {
            let col_x = panel_inner.x + 2 + (col_idx as u16) * (col_content_width + col_gap);
            let col_right_bound =
                (col_x + col_content_width).min(panel_inner.x + panel_inner.width - 2);
            let mut cy = content_y;

            if col_idx > 0 {
                let sep_x = col_x.saturating_sub(2);
                for row in content_y..max_y {
                    buf.set_string(
                        sep_x,
                        row,
                        "│",
                        Style::default().fg(self.theme.border_subtle),
                    );
                }
            }

            for (local_i, &si) in col_sections.iter().enumerate() {
                if cy >= max_y {
                    break;
                }
                let (section_name, items) = &sections[si];

                if local_i > 0 {
                    let sep_w = col_content_width.min(col_right_bound.saturating_sub(col_x));
                    let sep = "─".repeat(sep_w as usize);
                    buf.set_string(
                        col_x,
                        cy,
                        &sep,
                        Style::default().fg(self.theme.border_subtle),
                    );
                    cy += 1;
                    if cy >= max_y {
                        break;
                    }
                }

                buf.set_string(
                    col_x,
                    cy,
                    section_name,
                    Style::default()
                        .fg(self.theme.accent)
                        .add_modifier(Modifier::BOLD),
                );
                cy += 1;

                for (key, desc) in items {
                    if cy >= max_y {
                        break;
                    }
                    buf.set_string(
                        col_x + 1,
                        cy,
                        key,
                        Style::default()
                            .fg(self.theme.secondary)
                            .add_modifier(Modifier::BOLD),
                    );
                    let desc_x = col_x + 1 + max_key_width + 2;
                    if desc_x < col_right_bound {
                        let avail = col_right_bound.saturating_sub(desc_x) as usize;
                        let truncated: String = desc.chars().take(avail).collect();
                        buf.set_string(
                            desc_x,
                            cy,
                            &truncated,
                            Style::default().fg(self.theme.text_muted),
                        );
                    }
                    cy += 1;
                }
            }
        }

        let hint = "Press ? or Ctrl+/ to close";
        let hint_y = panel_inner.y + panel_inner.height.saturating_sub(1);
        if hint_y > content_y {
            let hint_x = inner_x;
            buf.set_string(
                hint_x,
                hint_y,
                hint,
                Style::default().fg(self.theme.text_muted),
            );
        }
    }
}
