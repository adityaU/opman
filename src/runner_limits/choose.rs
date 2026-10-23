//! How low a runner is, and where a session should go instead.
//!
//! The same rules the composer applies in the browser (`web-ui/src/runner-limit/`), so a
//! switch opman makes on its own lands where the user would have been offered: the user's
//! list in order, never a runner billed to the same account, never one that is out, and
//! one that is itself low only when the current runner is already out.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::runner::RunnerKind;
use crate::usage::types::{Account, AccountState, Window};
use crate::usage::{account_for, AccountId};

use super::settings::{FallbackPair, LimitSettings, ModelRef, NearPercent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Ok,
    Near,
    Out,
}

/// The accounts as last read, by id. A missing account reports nothing, which is not the
/// same as reporting room: such a runner is allowed, the way the composer allows it.
pub type Accounts = HashMap<AccountId, Account>;

fn left(window: &Window) -> f64 {
    (100.0 - window.used_percent).clamp(0.0, 100.0)
}

/// A window whose reset time has passed describes a window that no longer exists.
fn is_live(window: &Window, now: DateTime<Utc>) -> bool {
    window
        .resets_at
        .as_deref()
        .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
        .is_none_or(|at| at > now)
}

/// The level of one account, or `None` when its reading says nothing (signed out, an
/// error, or no live window).
pub fn account_level(account: &Account, near: NearPercent, now: DateTime<Utc>) -> Option<Level> {
    if account.state != AccountState::Ok {
        return None;
    }
    let tightest = account
        .windows
        .iter()
        .filter(|window| is_live(window, now))
        .map(left)
        .min_by(f64::total_cmp)?;
    Some(if tightest <= 0.0 {
        Level::Out
    } else if tightest <= near.get() {
        Level::Near
    } else {
        Level::Ok
    })
}

pub fn runner_level(
    kind: &RunnerKind,
    accounts: &Accounts,
    near: NearPercent,
    now: DateTime<Utc>,
) -> Option<Level> {
    accounts
        .get(&account_for(kind)?)
        .and_then(|account| account_level(account, near, now))
}

/// The first pair of the user's list that would get further than `current`, which is at
/// `current_level`.
pub fn choose<'a>(
    settings: &'a LimitSettings,
    current: &RunnerKind,
    current_level: Level,
    available: &[RunnerKind],
    accounts: &Accounts,
    now: DateTime<Utc>,
) -> Option<(RunnerKind, &'a FallbackPair)> {
    choose_for_model(
        settings,
        current,
        None,
        current_level,
        available,
        accounts,
        now,
    )
}

/// Match the current model's set before applying the account and usage checks.
pub fn choose_for_model<'a>(
    settings: &'a LimitSettings,
    current: &RunnerKind,
    model: Option<&ModelRef>,
    current_level: Level,
    available: &[RunnerKind],
    accounts: &Accounts,
    now: DateTime<Utc>,
) -> Option<(RunnerKind, &'a FallbackPair)> {
    if current_level == Level::Ok {
        return None;
    }
    let current_account = account_for(current);
    settings
        .candidates(&current.display_name(), model)
        .iter()
        .find_map(|pair| {
            // Matched by the name the browser shows, among the runners actually served.
            let kind = available
                .iter()
                .find(|kind| kind.display_name() == pair.runner.as_str())?;
            if kind == current {
                return None;
            }
            if current_account.is_some() && account_for(kind) == current_account {
                return None;
            }
            match runner_level(kind, accounts, settings.near_percent, now) {
                Some(Level::Out) => None,
                Some(Level::Near) if current_level != Level::Out => None,
                _ => Some((kind.clone(), pair)),
            }
        })
}

#[cfg(test)]
#[path = "choose_tests.rs"]
mod choose_tests;
