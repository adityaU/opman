//! The result types every check returns, and how they add up.

use serde::Serialize;

/// How one check ended. The order is the severity order, so the worst status is the maximum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Status {
    Pass,
    Warn,
    Fail,
}

/// One finding: a status, a one-line title, and the exact reason when there is one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Check {
    pub status: Status,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Check {
    fn new(status: Status, title: impl Into<String>) -> Self {
        Self {
            status,
            title: title.into(),
            detail: None,
        }
    }

    pub(crate) fn pass(title: impl Into<String>) -> Self {
        Self::new(Status::Pass, title)
    }

    pub(crate) fn warn(title: impl Into<String>) -> Self {
        Self::new(Status::Warn, title)
    }

    pub(crate) fn fail(title: impl Into<String>) -> Self {
        Self::new(Status::Fail, title)
    }

    /// Attach the exact underlying reason. Empty text is not a reason, so it is dropped.
    pub(crate) fn detail(mut self, detail: impl Into<String>) -> Self {
        let detail = detail.into();
        self.detail = Some(detail).filter(|text| !text.trim().is_empty());
        self
    }
}

/// A named group of checks that ran together.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Section {
    pub name: &'static str,
    pub checks: Vec<Check>,
}

impl Section {
    pub(crate) fn new(name: &'static str, checks: Vec<Check>) -> Self {
        Self { name, checks }
    }

    /// A section whose whole job did not finish in time.
    pub(crate) fn timed_out(name: &'static str, seconds: u64) -> Self {
        let title = format!("the {name} checks did not finish within {seconds} seconds");
        Self::new(name, vec![Check::fail(title)])
    }
}

/// How many checks ended in each status.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) struct Counts {
    pub pass: usize,
    pub warn: usize,
    pub fail: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Report {
    pub version: &'static str,
    pub sections: Vec<Section>,
}

impl Report {
    pub(crate) fn new(sections: Vec<Section>) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            sections,
        }
    }

    fn checks(&self) -> impl Iterator<Item = &Check> {
        self.sections
            .iter()
            .flat_map(|section| section.checks.iter())
    }

    pub(crate) fn counts(&self) -> Counts {
        self.checks().fold(Counts::default(), |mut counts, check| {
            match check.status {
                Status::Pass => counts.pass += 1,
                Status::Warn => counts.warn += 1,
                Status::Fail => counts.fail += 1,
            }
            counts
        })
    }

    /// The worst status of any check. An empty report passes.
    pub(crate) fn worst(&self) -> Status {
        self.checks()
            .map(|check| check.status)
            .max()
            .unwrap_or(Status::Pass)
    }

    /// 0 when nothing failed (warnings are fine), 1 otherwise.
    pub(crate) fn exit_code(&self) -> i32 {
        i32::from(self.worst() == Status::Fail)
    }
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod report_tests;
