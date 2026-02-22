use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VimMode {
    Insert,
    Normal,
    Command,
    WhichKey,
    Resize,
}

impl VimMode {
    pub fn label(&self) -> &'static str {
        match self {
            VimMode::Insert => "INSERT",
            VimMode::Normal => "NORMAL",
            VimMode::Command => "COMMAND",
            VimMode::WhichKey => "KEYS",
            VimMode::Resize => "RESIZE",
        }
    }
}

/// Detects rapid multi-press escape sequences for mode switching.
/// Terminal panels need double-Esc (single Esc is used by vim/tmux).
/// Non-terminal panels use triple-Esc (double Esc used by some terminal sequences).
pub struct EscapeTracker {
    timestamps: VecDeque<Instant>,
    threshold: Duration,
}

impl EscapeTracker {
    pub fn new() -> Self {
        Self {
            timestamps: VecDeque::with_capacity(4),
            threshold: Duration::from_millis(300),
        }
    }

    pub fn record_press(&mut self) {
        let now = Instant::now();
        self.timestamps.push_back(now);
        while self.timestamps.len() > 4 {
            self.timestamps.pop_front();
        }
    }

    /// Check if N rapid presses occurred within the threshold.
    /// Returns true and clears the tracker if triggered.
    pub fn check_multi_press(&mut self, count: usize) -> bool {
        if self.timestamps.len() < count {
            return false;
        }
        let recent: Vec<&Instant> = self.timestamps.iter().rev().take(count).collect();
        let oldest = recent.last().unwrap();
        let newest = recent.first().unwrap();
        if newest.duration_since(**oldest) <= self.threshold {
            self.timestamps.clear();
            return true;
        }
        false
    }

    /// For terminal panels: 2 rapid Esc presses.
    pub fn triggered_double(&mut self) -> bool {
        self.check_multi_press(2)
    }

    /// For non-terminal panels: 3 rapid Esc presses.
    pub fn triggered_triple(&mut self) -> bool {
        self.check_multi_press(3)
    }

    pub fn reset(&mut self) {
        self.timestamps.clear();
    }
}
