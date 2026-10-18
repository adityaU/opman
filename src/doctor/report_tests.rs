use super::*;

fn report(statuses: &[Status]) -> Report {
    let checks = statuses
        .iter()
        .map(|status| match status {
            Status::Pass => Check::pass("p"),
            Status::Warn => Check::warn("w"),
            Status::Fail => Check::fail("f"),
        })
        .collect();
    Report::new(vec![Section::new("one", checks)])
}

#[test]
fn an_empty_report_passes() {
    let empty = Report::new(Vec::new());
    assert_eq!(empty.worst(), Status::Pass);
    assert_eq!(empty.exit_code(), 0);
}

#[test]
fn warnings_do_not_fail_the_run() {
    let report = report(&[Status::Pass, Status::Warn]);
    assert_eq!(report.worst(), Status::Warn);
    assert_eq!(report.exit_code(), 0);
}

#[test]
fn one_failure_makes_the_exit_code_one() {
    let report = report(&[Status::Pass, Status::Fail, Status::Warn]);
    assert_eq!(report.worst(), Status::Fail);
    assert_eq!(report.exit_code(), 1);
}

#[test]
fn counts_add_up_across_sections() {
    let mut report = report(&[Status::Pass, Status::Warn]);
    report.sections.push(Section::new(
        "two",
        vec![Check::fail("f"), Check::pass("p")],
    ));
    assert_eq!(
        report.counts(),
        Counts {
            pass: 2,
            warn: 1,
            fail: 1
        }
    );
}

#[test]
fn a_blank_detail_is_dropped() {
    assert_eq!(Check::pass("x").detail("  \n").detail, None);
    assert_eq!(
        Check::pass("x").detail("why").detail.as_deref(),
        Some("why")
    );
}

#[test]
fn a_timed_out_section_is_one_failure() {
    let section = Section::timed_out("ACP agents", 60);
    assert_eq!(section.checks.len(), 1);
    assert_eq!(section.checks[0].status, Status::Fail);
}
