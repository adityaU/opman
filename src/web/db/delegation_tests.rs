//! Generated coverage tests for `db/delegation.rs`: update-row, status conversions.
use super::*;

fn item(id: &str, status: DelegationStatus, updated: &str) -> DelegatedWorkItem {
    DelegatedWorkItem {
        id: id.into(),
        title: format!("t-{id}"),
        assignee: "a".into(),
        scope: "s".into(),
        status,
        mission_id: Some("mi".into()),
        session_id: Some("se".into()),
        subagent_session_id: None,
        created_at: "2025-01-01T00:00:00Z".into(),
        updated_at: updated.into(),
    }
}

#[test]
fn list_sorted_desc_and_status_roundtrips() {
    let db = Db::open_memory().unwrap();
    db.insert_delegated_work(&item("p", DelegationStatus::Planned, "2025-01-01T00:00:00Z"));
    db.insert_delegated_work(&item("r", DelegationStatus::Running, "2025-03-01T00:00:00Z"));
    db.insert_delegated_work(&item("c", DelegationStatus::Completed, "2025-02-01T00:00:00Z"));
    let list = db.list_delegated_work();
    assert_eq!(list.iter().map(|d| d.id.clone()).collect::<Vec<_>>(), vec!["r", "c", "p"]);
    assert!(matches!(list[0].status, DelegationStatus::Running));
    assert!(matches!(list[1].status, DelegationStatus::Completed));
    assert!(matches!(list[2].status, DelegationStatus::Planned));
}

#[test]
fn update_row_found_and_not_found() {
    let db = Db::open_memory().unwrap();
    let mut d = item("u1", DelegationStatus::Planned, "2025-01-01T00:00:00Z");
    db.insert_delegated_work(&d);

    d.title = "renamed".into();
    d.status = DelegationStatus::Completed;
    d.subagent_session_id = Some("sub".into());
    d.updated_at = "2025-02-01T00:00:00Z".into();
    assert!(db.update_delegated_work_row(&d));
    let got = &db.list_delegated_work()[0];
    assert_eq!(got.title, "renamed");
    assert!(matches!(got.status, DelegationStatus::Completed));
    assert_eq!(got.subagent_session_id.as_deref(), Some("sub"));

    assert!(!db.update_delegated_work_row(&item("ghost", DelegationStatus::Planned, "2025-01-01T00:00:00Z")));
}

#[test]
fn delete_missing_is_false() {
    let db = Db::open_memory().unwrap();
    assert!(!db.delete_delegated_work_row("nope"));
}

#[test]
fn status_conversions() {
    assert_eq!(delegation_status_str(&DelegationStatus::Planned), "planned");
    assert_eq!(delegation_status_str(&DelegationStatus::Running), "running");
    assert_eq!(delegation_status_str(&DelegationStatus::Completed), "completed");
    assert!(matches!(parse_delegation_status("running"), DelegationStatus::Running));
    assert!(matches!(parse_delegation_status("completed"), DelegationStatus::Completed));
    assert!(matches!(parse_delegation_status("planned"), DelegationStatus::Planned));
    assert!(matches!(parse_delegation_status("weird"), DelegationStatus::Planned));
}
