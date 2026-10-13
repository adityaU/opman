//! `GET /api/usage` — what is left of each runner's provider allowance.

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use super::super::super::auth::AuthUser;
use super::super::super::types::ServerState;
use crate::usage::{self, UsageReport};

#[derive(Default, Deserialize)]
pub(crate) struct UsageQuery {
    refresh: Option<String>,
}

pub(crate) async fn get_usage(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<UsageQuery>,
) -> Json<UsageReport> {
    let refresh = matches!(query.refresh.as_deref(), Some("1" | "true"));
    let runners = state.runner_registry.available();
    Json(
        usage::global()
            .report(&state.http_client, &runners, refresh)
            .await,
    )
}
