//! How much of each runner's provider allowance is left, read live from the provider
//! the way the vendors' own CLIs do (`GET /api/usage`).
//!
//! Rules this module keeps: logins are only read, never refreshed (refresh tokens are
//! single-use, so refreshing would sign the user out of their CLI); a token is only
//! ever sent to its own provider; tokens are never logged or returned.

pub mod cache;
pub mod claude;
pub mod codex;
pub mod credentials;
pub mod rollout;
pub mod types;

use std::collections::BTreeMap;
use std::sync::LazyLock;
use std::time::Duration;

use reqwest::Client;

use crate::runner::RunnerKind;
use cache::Cached;
use claude::ClaudeSource;
use codex::CodexSource;
pub use types::{Account, AccountId, UsageReport};

const TTL: Duration = Duration::from_secs(60);
/// A failed read is retried this soon, so a network blip does not hide usage for a minute.
const ERROR_TTL: Duration = Duration::from_secs(10);
/// The most one account may hold up a request, login lookup included.
const BUDGET: Duration = Duration::from_secs(6);

/// Where each account's login lives and which server answers for it. Tests point these
/// at temp dirs and a fake server.
#[derive(Clone, Debug)]
pub struct Sources {
    pub claude: ClaudeSource,
    pub codex: CodexSource,
}

impl Sources {
    pub fn from_env() -> Self {
        Self {
            claude: ClaudeSource {
                base_url: claude::DEFAULT_BASE.to_string(),
                timeout: claude::TIMEOUT,
                config_dir: credentials::claude_dir(),
                keychain: credentials::use_keychain(),
            },
            codex: CodexSource {
                base_url: codex::DEFAULT_BASE.to_string(),
                timeout: codex::TIMEOUT,
                home: credentials::codex_home(),
            },
        }
    }
}

fn account_ttl(account: &Account) -> Duration {
    match account.state {
        types::AccountState::Error => ERROR_TTL,
        _ => TTL,
    }
}

pub struct UsageService {
    sources: Sources,
    claude: Cached<Account>,
    codex: Cached<Account>,
}

impl UsageService {
    pub fn new(sources: Sources) -> Self {
        Self {
            sources,
            claude: Cached::new(account_ttl),
            codex: Cached::new(account_ttl),
        }
    }

    /// Every account, fetched concurrently.
    pub async fn accounts(&self, client: &Client, refresh: bool) -> Vec<Account> {
        let (claude, codex) = tokio::join!(
            self.account(client, AccountId::Claude, refresh),
            self.account(client, AccountId::Codex, refresh),
        );
        vec![claude, codex]
    }

    pub async fn account(&self, client: &Client, id: AccountId, refresh: bool) -> Account {
        let cached = match id {
            AccountId::Claude => &self.claude,
            AccountId::Codex => &self.codex,
        };
        let fetch = || async {
            match id {
                AccountId::Claude => claude::account(client, &self.sources.claude).await,
                AccountId::Codex => codex::account(client, &self.sources.codex).await,
            }
        };
        tokio::time::timeout(BUDGET, cached.get(refresh, fetch))
            .await
            .unwrap_or_else(|_| Account::error(id, "Usage lookup took too long"))
    }

    pub async fn report(
        &self,
        client: &Client,
        runners: &[RunnerKind],
        refresh: bool,
    ) -> UsageReport {
        UsageReport {
            accounts: self.accounts(client, refresh).await,
            runners: runner_accounts(runners),
        }
    }
}

/// The process-wide service the route uses; the cache has to outlive a request.
pub fn global() -> &'static UsageService {
    static SERVICE: LazyLock<UsageService> =
        LazyLock::new(|| UsageService::new(Sources::from_env()));
    &SERVICE
}

/// The account a runner's turns are billed to, when opman can read it.
pub fn account_for(kind: &RunnerKind) -> Option<AccountId> {
    match kind {
        RunnerKind::Claude | RunnerKind::ClaudeCode => Some(AccountId::Claude),
        RunnerKind::Acp(id) if id == "claude" => Some(AccountId::Claude),
        RunnerKind::Acp(id) if id == "codex" => Some(AccountId::Codex),
        RunnerKind::Acp(_) | RunnerKind::Opencode => None,
    }
}

pub fn runner_accounts(runners: &[RunnerKind]) -> BTreeMap<String, AccountId> {
    runners
        .iter()
        .filter_map(|kind| Some((kind.display_name().into_owned(), account_for(kind)?)))
        .collect()
}

#[cfg(test)]
mod fake_provider;
#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
