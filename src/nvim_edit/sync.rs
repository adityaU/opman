//! Pure document bookkeeping shared by RPC and notification paths.

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Document {
    pub buffer: u64,
    pub path: String,
    pub changedtick: u64,
    pub lines: Vec<String>,
    pub attached: bool,
}

impl Document {
    pub(crate) fn new(buffer: u64, path: String, changedtick: u64, lines: Vec<String>) -> Self {
        Self {
            buffer,
            path,
            changedtick,
            lines,
            attached: true,
        }
    }

    pub(crate) fn require_tick(&self, expected: u64) -> Result<(), StaleTick> {
        (self.changedtick == expected)
            .then_some(())
            .ok_or(StaleTick {
                expected,
                actual: self.changedtick,
            })
    }

    pub(crate) fn apply_lines(
        &mut self,
        changedtick: u64,
        first_line: usize,
        last_line: usize,
        replacement: Vec<String>,
    ) -> Result<(), SyncError> {
        if first_line > last_line || last_line > self.lines.len() {
            return Err(SyncError::InvalidRange);
        }
        self.lines.splice(first_line..last_line, replacement);
        self.changedtick = changedtick;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StaleTick {
    pub expected: u64,
    pub actual: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SyncError {
    InvalidRange,
}

#[cfg(test)]
mod tests {
    use super::{Document, SyncError};

    #[test]
    fn line_range_mapping_replaces_only_the_reported_range() {
        let mut document = Document::new(
            3,
            "x".into(),
            7,
            vec!["one".into(), "two".into(), "three".into()],
        );
        document
            .apply_lines(8, 1, 2, vec!["2a".into(), "2b".into()])
            .expect("valid line event");
        assert_eq!(document.lines, ["one", "2a", "2b", "three"]);
        assert_eq!(document.changedtick, 8);
    }

    #[test]
    fn invalid_line_ranges_are_rejected() {
        let mut document = Document::new(1, "x".into(), 1, vec!["x".into()]);
        assert_eq!(
            document.apply_lines(2, 0, 2, Vec::new()),
            Err(SyncError::InvalidRange)
        );
    }

    #[test]
    fn stale_changedticks_are_rejected_without_mutating_document() {
        let document = Document::new(1, "x".into(), 12, vec!["x".into()]);
        let result = document.require_tick(11);
        assert_eq!(result.unwrap_err().actual, 12);
        assert_eq!(document.lines, ["x"]);
    }
}
