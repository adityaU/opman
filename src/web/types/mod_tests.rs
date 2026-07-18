use super::*;

// mod.rs only declares submodules and re-exports (`pub use`) them; there is no
// executable logic here. This test simply references a few re-exported items to
// confirm the glob re-exports resolve.
#[test]
fn reexports_are_reachable() {
    // Types re-exported from several submodules.
    let stage = PipelineStage {
        lane_id: "l".into(),
        session_id: None,
        status: "pending".into(),
        output: None,
    };
    assert_eq!(stage.lane_id, "l");

    let scope = MemoryScope::Global;
    assert_eq!(serde_json::to_value(scope).unwrap(), "global");

    let layout = WorkspaceLayout::default();
    assert_eq!(layout.sidebar_width, 0);

    let net = NetworkInfo {
        name: "lo".into(),
        rx_bytes: 0,
        tx_bytes: 0,
    };
    assert_eq!(net.name, "lo");
}
