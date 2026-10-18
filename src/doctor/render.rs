//! The human report and the JSON report.

use std::fmt::Write;

use serde_json::{json, Value};

use super::report::{Check, Report, Status};

const DETAIL_INDENT: &str = "      ";

struct Style {
    color: bool,
}

impl Style {
    fn paint(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    fn marker(&self, status: Status) -> String {
        match status {
            Status::Pass => self.paint("32", "✔"),
            Status::Warn => self.paint("33", "⚠"),
            Status::Fail => self.paint("31", "✖"),
        }
    }
}

/// The report as text. `color` adds ANSI codes; leave it off when stdout is not a terminal.
pub(crate) fn human(report: &Report, color: bool) -> String {
    let style = Style { color };
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{}",
        style.paint("1", &format!("opman doctor (opman {})", report.version))
    );
    for section in &report.sections {
        let _ = writeln!(out, "\n{}", style.paint("1", section.name));
        for check in &section.checks {
            line(&mut out, &style, check);
        }
    }
    let counts = report.counts();
    let _ = writeln!(
        out,
        "\n{} passed, {} warnings, {} failed",
        counts.pass, counts.warn, counts.fail
    );
    out
}

fn line(out: &mut String, style: &Style, check: &Check) {
    let _ = writeln!(out, "  {} {}", style.marker(check.status), check.title);
    let Some(detail) = &check.detail else {
        return;
    };
    for text in detail.lines() {
        let _ = writeln!(out, "{DETAIL_INDENT}{text}");
    }
}

/// The report as one JSON document.
pub(crate) fn json(report: &Report) -> Value {
    json!({
        "version": report.version,
        "ok": report.exit_code() == 0,
        "summary": report.counts(),
        "sections": report.sections,
    })
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod render_tests;
