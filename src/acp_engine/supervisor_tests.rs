//! Tests for the reconcile pass.
//!
//! `retire` is the half that decides, and it is pure — which is why it is a free function
//! rather than a method. Starting an engine binds a port and spawns a task, so the deciding
//! is tested here and the doing is left to the integration surface.

use super::*;

use std::collections::BTreeMap;

/// A default runner that no test agent occupies, so pinning is out of the way unless a
/// test asks for it.
fn unpinned() -> RunnerKind {
    RunnerKind::Opencode
}

fn agent(command: &str) -> AgentConfig {
    AgentConfig {
        command: command.to_string(),
        runner: "gemini".to_string(),
        ..AgentConfig::default()
    }
}

fn live_with(id: &str, config: AgentConfig) -> HashMap<String, Live> {
    // The engine is never touched by `retire`; it only decides which entries leave.
    let engine = Arc::new(AcpEngine::new(
        id.to_string(),
        config.clone(),
        None,
        crate::mcp_registry::SharedRegistry::default(),
    ));
    HashMap::from([(
        id.to_string(),
        Live {
            kind: RunnerKind::Acp(id.to_string()),
            config,
            engine,
        },
    )])
}

fn config_with(id: &str, entry: AgentConfig) -> AcpConfig {
    AcpConfig {
        agents: BTreeMap::from([(id.to_string(), entry)]),
    }
}

#[test]
fn an_unchanged_agent_is_left_running() {
    let mut live = live_with("gemini", agent("gemini-acp"));
    let (retired, deferred) = retire(
        &mut live,
        &config_with("gemini", agent("gemini-acp")),
        &unpinned(),
    );
    // Restarting on every save would kill a turn in flight for no reason.
    assert!(retired.is_empty());
    assert!(deferred.is_empty());
    assert!(live.contains_key("gemini"));
}

#[test]
fn an_edited_agent_is_retired_so_it_can_be_restarted() {
    let mut live = live_with("gemini", agent("gemini-acp"));
    let (retired, _) = retire(
        &mut live,
        &config_with("gemini", agent("other-acp")),
        &unpinned(),
    );
    // The launch command is fixed when the child spawns, so a new one means a new process.
    assert_eq!(retired.len(), 1);
    assert!(live.is_empty());
}

#[test]
fn a_disabled_agent_is_retired() {
    let mut live = live_with("gemini", agent("gemini-acp"));
    let disabled = AgentConfig {
        enabled: false,
        ..agent("gemini-acp")
    };
    let (retired, _) = retire(&mut live, &config_with("gemini", disabled), &unpinned());
    assert_eq!(retired.len(), 1);
}

#[test]
fn an_agent_deleted_from_config_is_retired() {
    let mut live = live_with("gemini", agent("gemini-acp"));
    let (retired, _) = retire(&mut live, &AcpConfig::default(), &unpinned());
    assert_eq!(retired.len(), 1);
    assert_eq!(retired[0].kind, RunnerKind::Acp("gemini".to_string()));
}

/// The claim the settings page makes: a config edit takes effect on the running process.
///
/// Nothing is spawned by installing an agent — an ACP child starts with the first session —
/// so this can drive the real reconcile against a real registry and check the runner slot
/// it is supposed to fill.
#[tokio::test]
async fn reconciling_installs_and_uninstalls_the_runner() {
    let registry = Arc::new(crate::runner::RunnerRegistry::new(
        RunnerKind::Opencode,
        HashMap::new(),
    ));
    let supervisor = AcpSupervisor::adopt(
        registry.clone(),
        crate::mcp_registry::SharedRegistry::default(),
        reqwest::Client::new(),
        std::iter::empty(),
    );
    let slot = RunnerKind::Acp("gemini".to_string());

    let added = supervisor
        .reconcile(&config_with("gemini", agent("/bin/true")))
        .await;
    assert_eq!(added.added, vec![slot.clone()]);
    assert!(
        registry.has(&slot),
        "the agent must be selectable as a runner"
    );

    let removed = supervisor.reconcile(&AcpConfig::default()).await;
    assert_eq!(removed.removed, vec![slot.clone()]);
    assert!(
        !registry.has(&slot),
        "a deleted agent must stop being offered"
    );
}

/// An ACP agent must not be able to take a slot another engine serves — `opencode` and
/// `claude-code` are not ACP, and displacing one would strand every session bound to it.
#[tokio::test]
async fn an_occupied_slot_is_reported_rather_than_seized() {
    let mut runners: HashMap<RunnerKind, Arc<dyn crate::runner::Runner>> = HashMap::new();
    runners.insert(
        RunnerKind::Opencode,
        Arc::new(crate::runner::HttpRunner::new(
            RunnerKind::Opencode,
            "http://127.0.0.1:9",
            reqwest::Client::new(),
        )),
    );
    let registry = Arc::new(crate::runner::RunnerRegistry::new(
        RunnerKind::Opencode,
        runners,
    ));
    let supervisor = AcpSupervisor::adopt(
        registry.clone(),
        crate::mcp_registry::SharedRegistry::default(),
        reqwest::Client::new(),
        std::iter::empty(),
    );

    let squatter = AgentConfig {
        runner: "opencode".to_string(),
        ..agent("/bin/true")
    };
    let changes = supervisor
        .reconcile(&config_with("squatter", squatter))
        .await;
    assert_eq!(changes.blocked, vec!["squatter".to_string()]);
    assert!(changes.added.is_empty());
}

/// The default runner's engine published its URL to the TUI once at startup, so restarting
/// it would leave the TUI talking to a closed port. Its edit waits rather than being
/// applied — and is reported, so the page can say the change needs a restart.
#[test]
fn the_default_runners_agent_is_spared_and_reported() {
    let mut live = live_with("gemini", agent("gemini-acp"));
    let pinned = RunnerKind::Acp("gemini".to_string());
    let (retired, deferred) = retire(
        &mut live,
        &config_with("gemini", agent("other-acp")),
        &pinned,
    );
    assert!(retired.is_empty());
    assert_eq!(deferred, vec!["gemini".to_string()]);
    assert!(
        live.contains_key("gemini"),
        "it must keep running on the old definition"
    );
}

/// A slot reserved by an agent's own not-yet-started lazy runner is not a clash. Without
/// this every configured agent would report as blocked from the moment runners became
/// lazy, because each one holds a placeholder in the registry from boot.
#[tokio::test]
async fn a_slot_reserved_by_its_own_lazy_runner_is_not_blocked() {
    let slot = RunnerKind::Acp("gemini".to_string());
    register_acp_runners(std::iter::once("gemini".to_string()));
    let lazy = crate::runner_lazy::LazyRunner::new(
        slot.clone(),
        crate::runner_lazy::LazyContext::new(),
        || async { anyhow::bail!("never started in this test") },
    )
    .for_acp_agent("gemini");
    let runners: HashMap<RunnerKind, Arc<dyn crate::runner::Runner>> = HashMap::from([(
        slot.clone(),
        Arc::new(lazy) as Arc<dyn crate::runner::Runner>,
    )]);
    let registry = Arc::new(crate::runner::RunnerRegistry::new(unpinned(), runners));
    let supervisor = AcpSupervisor::adopt(
        registry.clone(),
        crate::mcp_registry::SharedRegistry::default(),
        reqwest::Client::new(),
        std::iter::empty(),
    );

    // `/bin/true` exits immediately, so the start fails rather than leaving a child
    // behind — enough to prove the agent was attempted rather than reported blocked.
    let changes = supervisor
        .reconcile(&config_with("gemini", agent("/bin/true")))
        .await;
    assert!(
        changes.blocked.is_empty(),
        "its own reservation is not a clash"
    );
}

/// An engine a lazy runner starts has to end up owned by the supervisor, or every later
/// reconcile would see a slot it did not create and refuse to touch it.
#[tokio::test]
async fn adopt_one_takes_ownership_of_a_first_use_start() {
    let slot = RunnerKind::Acp("gemini".to_string());
    register_acp_runners(std::iter::once("gemini".to_string()));
    let registry = Arc::new(crate::runner::RunnerRegistry::new(
        unpinned(),
        HashMap::new(),
    ));
    let supervisor = AcpSupervisor::adopt(
        registry,
        crate::mcp_registry::SharedRegistry::default(),
        reqwest::Client::new(),
        std::iter::empty(),
    );
    assert!(supervisor.running().await.is_empty());

    let config = agent("gemini-acp");
    let engine = Arc::new(AcpEngine::new(
        "gemini".to_string(),
        config.clone(),
        None,
        crate::mcp_registry::SharedRegistry::default(),
    ));
    supervisor
        .adopt_one("gemini".to_string(), slot.clone(), engine)
        .await;

    assert_eq!(supervisor.running().await.get("gemini"), Some(&slot));
    // Owned means reconcilable: an unchanged config now leaves it alone.
    let changes = supervisor.reconcile(&config_with("gemini", config)).await;
    assert!(changes.added.is_empty() && changes.removed.is_empty());
}
