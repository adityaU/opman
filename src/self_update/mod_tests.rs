use super::*;

fn v(s: &str) -> Version {
    s.parse().expect("valid test fixture")
}

#[test]
fn update_installs_only_a_newer_release() {
    assert_eq!(
        plan_update(v("0.10.2"), v("0.11.0")),
        UpdatePlan::Install(v("0.11.0"))
    );
    assert_eq!(plan_update(v("0.10.2"), v("0.10.2")), UpdatePlan::UpToDate);
    assert_eq!(plan_update(v("0.10.2"), v("0.10.1")), UpdatePlan::UpToDate);
}

#[test]
fn downgrade_accepts_only_older_versions() {
    assert!(check_downgrade(v("0.10.2"), v("0.10.1")).is_ok());
    assert!(check_downgrade(v("0.10.2"), v("0.9.9")).is_ok());
}

#[test]
fn downgrade_refuses_same_or_newer_and_points_to_update() {
    for wanted in ["0.10.2", "0.10.3", "1.0.0"] {
        let err = check_downgrade(v("0.10.2"), v(wanted))
            .expect_err("invalid fixture must fail")
            .to_string();
        assert!(err.contains("opman update --version"), "{err}");
    }
}
