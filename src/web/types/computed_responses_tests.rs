use super::*;
use crate::web::types::{
    InboxItemPriority, InboxItemSource, InboxItemState, RecommendationAction, SignalInput,
    WorkspaceLayout, WorkspacePanels,
};
use serde_json::json;

#[test]
fn inbox_item_serializes_with_optionals() {
    let item = InboxItem {
        id: "i1".into(),
        source: InboxItemSource::Permission,
        title: "t".into(),
        description: "d".into(),
        priority: InboxItemPriority::High,
        state: InboxItemState::Unresolved,
        created_at: 12.0,
        session_id: Some("s".into()),
        mission_id: Some("m".into()),
    };
    let v = serde_json::to_value(&item).unwrap();
    assert_eq!(v["source"], "permission");
    assert_eq!(v["priority"], "high");
    assert_eq!(v["state"], "unresolved");
    assert_eq!(v["session_id"], "s");
    assert_eq!(v["mission_id"], "m");
    let _ = format!("{item:?}");
    let _ = item.clone();
}

#[test]
fn inbox_item_omits_none_optionals() {
    let item = InboxItem {
        id: "i".into(),
        source: InboxItemSource::Completion,
        title: "t".into(),
        description: "d".into(),
        priority: InboxItemPriority::Low,
        state: InboxItemState::Informational,
        created_at: 0.0,
        session_id: None,
        mission_id: None,
    };
    let v = serde_json::to_value(&item).unwrap();
    assert!(v.get("session_id").is_none());
    assert!(v.get("mission_id").is_none());
}

#[test]
fn inbox_response_serializes() {
    let resp = InboxResponse { items: vec![] };
    assert_eq!(serde_json::to_value(&resp).unwrap()["items"], json!([]));
    let _ = format!("{resp:?}");
    let _ = resp.clone();
}

#[test]
fn assistant_recommendation_serializes() {
    let r = AssistantRecommendation {
        id: "r".into(),
        title: "t".into(),
        rationale: "why".into(),
        action: RecommendationAction::OpenInbox,
        priority: InboxItemPriority::Medium,
    };
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["action"], "open_inbox");
    assert_eq!(v["priority"], "medium");
    let _ = format!("{r:?}");
    let _ = r.clone();
}

#[test]
fn recommendations_response_serializes() {
    let resp = RecommendationsResponse {
        recommendations: vec![],
    };
    assert_eq!(
        serde_json::to_value(&resp).unwrap()["recommendations"],
        json!([])
    );
    let _ = format!("{resp:?}");
    let _ = resp.clone();
}

#[test]
fn handoff_link_omits_none_source_id() {
    let l = HandoffLink {
        kind: "session".into(),
        label: "L".into(),
        source_id: None,
    };
    let v = serde_json::to_value(&l).unwrap();
    assert!(v.get("source_id").is_none());
    let l2 = HandoffLink {
        kind: "k".into(),
        label: "l".into(),
        source_id: Some("id".into()),
    };
    assert_eq!(serde_json::to_value(&l2).unwrap()["source_id"], "id");
    let _ = format!("{l:?}");
    let _ = l.clone();
}

#[test]
fn handoff_brief_serializes() {
    let b = HandoffBrief {
        title: "t".into(),
        summary: "s".into(),
        blockers: vec!["b1".into()],
        recent_changes: vec!["c1".into()],
        next_action: "go".into(),
        links: vec![HandoffLink {
            kind: "k".into(),
            label: "l".into(),
            source_id: None,
        }],
    };
    let v = serde_json::to_value(&b).unwrap();
    assert_eq!(v["blockers"], json!(["b1"]));
    assert_eq!(v["recent_changes"], json!(["c1"]));
    assert_eq!(v["links"].as_array().unwrap().len(), 1);
    let _ = format!("{b:?}");
    let _ = b.clone();
}

#[test]
fn resume_briefing_serializes() {
    let r = ResumeBriefing {
        title: "t".into(),
        summary: "s".into(),
        next_action: "n".into(),
    };
    assert_eq!(serde_json::to_value(&r).unwrap()["next_action"], "n");
    let _ = format!("{r:?}");
    let _ = r.clone();
}

#[test]
fn daily_summary_response_serializes() {
    let r = DailySummaryResponse {
        summary: "sum".into(),
    };
    assert_eq!(serde_json::to_value(&r).unwrap()["summary"], "sum");
    let _ = format!("{r:?}");
    let _ = r.clone();
}

#[test]
fn signals_response_serializes() {
    let r = SignalsResponse {
        signals: vec![SignalInput {
            id: "s".into(),
            kind: "k".into(),
            title: "t".into(),
            body: "b".into(),
            created_at: 1.0,
            session_id: None,
        }],
    };
    assert_eq!(
        serde_json::to_value(&r).unwrap()["signals"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let _ = format!("{r:?}");
    let _ = r.clone();
}

#[test]
fn assistant_center_stats_serializes() {
    let s = AssistantCenterStats {
        active_missions: 1,
        paused_missions: 2,
        total_missions: 3,
        pending_permissions: 4,
        pending_questions: 5,
        memory_items: 6,
        active_routines: 7,
        active_delegations: 8,
        workspace_count: 9,
        autonomy_mode: "nudge".into(),
    };
    let v = serde_json::to_value(&s).unwrap();
    assert_eq!(v["active_missions"], 1);
    assert_eq!(v["workspace_count"], 9);
    assert_eq!(v["autonomy_mode"], "nudge");
    let _ = format!("{s:?}");
    let _ = s.clone();
}

#[test]
fn workspace_template_serializes() {
    let t = WorkspaceTemplate {
        id: "t".into(),
        name: "n".into(),
        description: "d".into(),
        panels: WorkspacePanels {
            sidebar: true,
            terminal: false,
            editor: true,
            git: false,
        },
        layout: WorkspaceLayout {
            sidebar_width: 100,
            terminal_height: 200,
            side_panel_width: 300,
        },
    };
    let v = serde_json::to_value(&t).unwrap();
    assert_eq!(v["panels"]["sidebar"], true);
    assert_eq!(v["panels"]["terminal"], false);
    assert_eq!(v["layout"]["sidebar_width"], 100);
    assert_eq!(v["layout"]["side_panel_width"], 300);
    let _ = format!("{t:?}");
    let _ = t.clone();
}

#[test]
fn workspace_templates_response_serializes() {
    let resp = WorkspaceTemplatesResponse { templates: vec![] };
    assert_eq!(serde_json::to_value(&resp).unwrap()["templates"], json!([]));
    let _ = format!("{resp:?}");
    let _ = resp.clone();
}
