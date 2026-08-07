//! Tests for autonomy settings.
use super::*;
use crate::web::web_state::WebStateHandle;

// ── Autonomy ────────────────────────────────────────────────────────

#[tokio::test]
async fn autonomy_get_default_and_update() {
    let h = WebStateHandle::new_test();
    // Default is retrievable.
    let _ = h.get_autonomy_settings().await;
    let updated = h.update_autonomy_settings(AutonomyMode::Autonomous).await;
    assert!(matches!(updated.mode, AutonomyMode::Autonomous));
    let got = h.get_autonomy_settings().await;
    assert!(matches!(got.mode, AutonomyMode::Autonomous));

    // Cover the other modes too.
    for mode in [
        AutonomyMode::Observe,
        AutonomyMode::Nudge,
        AutonomyMode::Continue,
    ] {
        let out = h.update_autonomy_settings(mode).await;
        assert!(!out.updated_at.is_empty());
    }
}
