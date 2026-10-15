//! Where a session goes: the user's list in order, never the same account, never a
//! runner that is out, and a low one only when the current runner is out.

use chrono::{Duration, Utc};

use super::*;
use crate::runner_limits::settings::{FallbackPair, LimitPolicy, LimitSettings, NearPercent};
use crate::usage::types::Window;

fn account(id: AccountId, used: f64) -> Account {
    let mut account = Account::new(id, AccountState::Ok);
    let reset = (Utc::now() + Duration::hours(3)).to_rfc3339();
    account.windows = vec![Window::new("5h", "5-hour", used).resets_at(Some(reset))];
    account
}

fn accounts(claude: f64, codex: f64) -> Accounts {
    [account(AccountId::Claude, claude), account(AccountId::Codex, codex)]
        .into_iter()
        .map(|account| (account.id, account))
        .collect()
}

fn settings(pairs: &[&str]) -> LimitSettings {
    LimitSettings {
        policy: LimitPolicy::Auto,
        near_percent: NearPercent::DEFAULT,
        pairs: pairs
            .iter()
            .map(|runner| FallbackPair { runner: (*runner).to_string(), model: None })
            .collect(),
    }
}

fn available() -> Vec<RunnerKind> {
    vec![RunnerKind::Claude, RunnerKind::ClaudeCode, RunnerKind::Acp("codex".into()), RunnerKind::Opencode]
}

#[test]
fn the_level_follows_the_threshold() {
    let now = Utc::now();
    let near = NearPercent::DEFAULT;
    assert_eq!(account_level(&account(AccountId::Claude, 90.0), near, now), Some(Level::Ok));
    assert_eq!(account_level(&account(AccountId::Claude, 96.0), near, now), Some(Level::Near));
    assert_eq!(account_level(&account(AccountId::Claude, 100.0), near, now), Some(Level::Out));
    // A signed-out account says nothing about room.
    let signed_out = Account::new(AccountId::Claude, AccountState::SignedOut);
    assert_eq!(account_level(&signed_out, near, now), None);
}

#[test]
fn a_window_that_already_reset_is_ignored() {
    let mut spent = account(AccountId::Claude, 100.0);
    spent.windows[0].resets_at = Some((Utc::now() - Duration::minutes(1)).to_rfc3339());
    assert_eq!(account_level(&spent, NearPercent::DEFAULT, Utc::now()), None);
}

#[test]
fn nothing_moves_while_the_runner_has_room() {
    let list = settings(&["codex"]);
    let picked = choose(&list, &RunnerKind::Claude, Level::Ok, &available(), &accounts(10.0, 10.0), Utc::now());
    assert!(picked.is_none());
}

#[test]
fn the_first_pair_with_room_wins_and_the_same_account_is_skipped() {
    let list = settings(&["claude-code", "codex", "opencode"]);
    let picked = choose(&list, &RunnerKind::Claude, Level::Near, &available(), &accounts(96.0, 30.0), Utc::now());
    assert_eq!(picked.map(|(kind, _)| kind), Some(RunnerKind::Acp("codex".into())));
}

#[test]
fn a_low_target_is_used_only_when_the_current_runner_is_out() {
    let list = settings(&["codex", "opencode"]);
    let both_low = accounts(96.0, 97.0);
    let near = choose(&list, &RunnerKind::Claude, Level::Near, &available(), &both_low, Utc::now());
    // opencode reports no usage: allowed, after the low one is passed over.
    assert_eq!(near.map(|(kind, _)| kind), Some(RunnerKind::Opencode));
    let out = choose(&list, &RunnerKind::Claude, Level::Out, &available(), &both_low, Utc::now());
    assert_eq!(out.map(|(kind, _)| kind), Some(RunnerKind::Acp("codex".into())));
}

#[test]
fn a_runner_that_is_not_served_or_is_out_is_skipped() {
    let list = settings(&["gone", "codex"]);
    let picked = choose(&list, &RunnerKind::Claude, Level::Out, &available(), &accounts(100.0, 100.0), Utc::now());
    assert!(picked.is_none());
}
