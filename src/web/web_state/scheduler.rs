//! Cron-based routine scheduler.
//!
//! Spawns a background task that checks enabled scheduled routines
//! every 30 seconds and fires any whose `next_run_at` has passed.

use chrono::{DateTime, Utc};
use cron::Schedule;
use std::str::FromStr;
use tracing::{debug, info, warn};

use super::super::types::*;

impl super::WebStateHandle {
    /// Spawn the routine scheduler background task.
    ///
    /// Checks every 30 seconds for scheduled routines whose `next_run_at`
    /// is in the past (or unset). Fires them and computes the next run time.
    pub(super) fn spawn_routine_scheduler(&self) {
        let handle = self.clone();

        tokio::spawn(async move {
            // Wait for server to be ready before starting scheduler
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;

            info!("routine scheduler started");

            // On startup, compute next_run_at for all scheduled routines that need it
            handle.recompute_all_next_runs().await;

            loop {
                handle.tick_scheduler().await;
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            }
        });
    }

    /// Single scheduler tick: find due routines and execute them.
    async fn tick_scheduler(&self) {
        let now = Utc::now();

        // Collect routines that are due
        let due_routines: Vec<String> = {
            let state = self.inner.read().await;
            state
                .routines
                .values()
                .filter(|r| {
                    r.enabled
                        && r.trigger == RoutineTrigger::Scheduled
                        && r.cron_expr.is_some()
                })
                .filter(|r| {
                    // Check if next_run_at is in the past (or not set)
                    match r.next_run_at.as_deref() {
                        Some(ts) => {
                            DateTime::parse_from_rfc3339(ts)
                                .map(|dt| dt <= now)
                                .unwrap_or(true)
                        }
                        None => true, // Never computed — treat as due
                    }
                })
                .map(|r| r.id.clone())
                .collect()
        };

        for routine_id in due_routines {
            debug!(routine_id = %routine_id, "executing scheduled routine");

            match self.execute_routine(&routine_id).await {
                Ok(run) => {
                    debug!(
                        routine_id = %routine_id,
                        run_id = %run.id,
                        "scheduled routine executed successfully"
                    );
                }
                Err(e) => {
                    warn!(
                        routine_id = %routine_id,
                        error = %e,
                        "scheduled routine execution failed"
                    );
                }
            }

            // Compute next run time
            self.update_next_run(&routine_id).await;
        }
    }

    /// Recompute `next_run_at` for all scheduled routines.
    async fn recompute_all_next_runs(&self) {
        let routine_ids: Vec<String> = {
            let state = self.inner.read().await;
            state
                .routines
                .values()
                .filter(|r| {
                    r.enabled
                        && r.trigger == RoutineTrigger::Scheduled
                        && r.cron_expr.is_some()
                })
                .map(|r| r.id.clone())
                .collect()
        };

        for id in routine_ids {
            self.update_next_run(&id).await;
        }
    }

    /// Update the `next_run_at` field for a routine based on its cron expression.
    async fn update_next_run(&self, routine_id: &str) {
        let mut state = self.inner.write().await;
        let Some(routine) = state.routines.get_mut(routine_id) else {
            return;
        };

        let Some(cron_expr) = routine.cron_expr.as_deref() else {
            routine.next_run_at = None;
            return;
        };

        match compute_next_run(cron_expr, routine.timezone.as_deref()) {
            Some(next) => {
                routine.next_run_at = Some(next.to_rfc3339());
            }
            None => {
                routine.next_run_at = None;
                warn!(
                    routine_id = %routine_id,
                    cron_expr = %cron_expr,
                    "could not compute next run time"
                );
            }
        }
        drop(state);
        self.schedule_persist();
    }

    /// Immediately recompute `next_run_at` for a single routine if it is
    /// enabled, scheduled, and has a cron expression.  Called after routine
    /// updates so the scheduler does not wait for the next 30-second tick.
    pub(super) async fn recompute_next_run_if_scheduled(&self, routine_id: &str) {
        let needs_update = {
            let state = self.inner.read().await;
            state.routines.get(routine_id).map_or(false, |r| {
                r.enabled && r.trigger == RoutineTrigger::Scheduled && r.cron_expr.is_some()
            })
        };
        if needs_update {
            self.update_next_run(routine_id).await;
        }
    }
}

/// Parse a cron expression and compute the next fire time.
///
/// The `cron` crate uses 7-field expressions (sec min hour dom month dow year).
/// We accept 5-field user input (min hour dom month dow) and prepend "0 " and
/// append " *" to get the 7-field format.
fn compute_next_run(
    cron_expr: &str,
    timezone: Option<&str>,
) -> Option<DateTime<Utc>> {
    // Normalize to the 7-field cron format: sec min hour dom month dow year
    // The frontend sends 6-field expressions: sec min hour dom month dow
    // Standard 5-field (min hour dom month dow) gets sec=0 prepended + year=* appended
    let parts: Vec<&str> = cron_expr.split_whitespace().collect();
    let full_expr = match parts.len() {
        5 => format!("0 {} *", cron_expr),   // prepend sec=0, append year=*
        6 => format!("{} *", cron_expr),      // append year=*
        7 => cron_expr.to_string(),           // already 7-field
        _ => return None,
    };

    let schedule = Schedule::from_str(&full_expr).ok()?;

    // If a timezone is specified, compute in that timezone then convert to UTC
    if let Some(tz_name) = timezone {
        if let Ok(tz) = tz_name.parse::<chrono_tz::Tz>() {
            let now_in_tz = Utc::now().with_timezone(&tz);
            return schedule.after(&now_in_tz).next().map(|dt| dt.with_timezone(&Utc));
        }
    }

    // Default: compute in UTC
    schedule.after(&Utc::now()).next().map(|dt| dt.with_timezone(&Utc))
}
