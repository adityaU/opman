//! Web state construction and test constructors.

use super::*;

impl WebStateHandle {
    /// Create the web state from config, start background pollers.
    ///
    /// `event_tx` is the broadcast channel that SSE clients subscribe to.
    /// `raw_sse_tx` is the broadcast channel for re-broadcasting raw upstream
    /// opencode SSE events to web clients.
    pub fn new(
        config: &Config,
        event_tx: broadcast::Sender<WebEvent>,
        raw_sse_tx: broadcast::Sender<String>,
        runner_registry: Arc<crate::runner::RunnerRegistry>,
    ) -> Self {
        // Open SQLite database and run one-time migration from legacy JSON.
        let db = Db::open().unwrap_or_else(|e| {
            panic!("failed to open assistant database: {e}");
        });
        crate::web::db::migrate_legacy_json::run_migration(&db);

        let projects: Vec<WebProject> = config
            .projects
            .iter()
            .map(|entry| WebProject {
                name: entry.name.clone(),
                path: PathBuf::from(&entry.path),
                sessions: Vec::new(),
                active_session: None,
                git_branch: String::new(),
            })
            .collect();

        let inner = Arc::new(RwLock::new(Self::build_inner(&db, projects)));

        let (persist_tx, persist_rx) = mpsc::unbounded_channel();

        let handle = Self {
            inner,
            event_tx,
            raw_sse_tx,
            editor_tx: None,
            db,
            persist_tx,
            runner_registry: Some(runner_registry),
        };

        handle.register_runner_started_hook();

        // Spawn background tasks
        handle.spawn_persist_worker(persist_rx);
        handle.spawn_session_poller();
        handle.spawn_status_poller();
        handle.spawn_opencode_sse_listener();
        handle.spawn_routine_scheduler();
        handle.spawn_presence_cleanup();

        handle
    }

    /// Build the inner mutable state, loading persisted collections from `db`.
    /// Shared by the production constructor and the test constructors.
    pub(super) fn build_inner(db: &Db, projects: Vec<WebProject>) -> WebStateInner {
        let personal_memory: HashMap<String, PersonalMemoryItem> = db
            .list_memory()
            .into_iter()
            .map(|m| (m.id.clone(), m))
            .collect();
        let autonomy_settings = db.load_autonomy_settings();
        let routines: HashMap<String, RoutineDefinition> = db
            .list_routines()
            .into_iter()
            .map(|r| (r.id.clone(), r))
            .collect();
        let routine_runs = db.list_routine_runs();

        WebStateInner {
            startup_hydration: background_hydration::StartupHydration::Pending,
            active_project: 0,
            projects,
            panels: WebPanelVisibility {
                sidebar: true,
                terminal_pane: true,
                neovim_pane: true,
                integrated_terminal: true,
                git_panel: true,
            },
            focused: "TerminalPane".to_string(),
            session_stats: HashMap::new(),
            busy_sessions: HashSet::new(),
            turn_dispatch: HashMap::new(),
            theme: None,
            session_watchers: HashMap::new(),
            watcher_pending: HashMap::new(),
            watcher_idle_since: HashMap::new(),
            session_children: HashMap::new(),
            file_snapshots: HashMap::new(),
            file_edits: HashMap::new(),
            connected_clients: HashMap::new(),
            personal_memory,
            autonomy_settings,
            routines,
            routine_runs,
            pending_permissions: HashMap::new(),
            pending_questions: HashMap::new(),
            error_sessions: HashMap::new(),
            input_sessions: HashSet::new(),
            unseen_sessions: HashMap::new(),
            routine_idle_cooldown: HashMap::new(),
            session_runners: HashMap::new(),
            instructions_sent: HashSet::new(),
            default_runner: "opencode".to_string(),
        }
    }

    /// Test-only constructor: in-memory DB, no background pollers/scheduler.
    ///
    /// The persist channel receiver is dropped, so `mark_dirty()` calls are
    /// silently discarded (no persistence worker runs).
    #[cfg(test)]
    pub(crate) fn new_test() -> Self {
        Self::new_test_with_projects(Vec::new())
    }

    /// Test-only constructor with pre-populated projects (`(name, path)`).
    #[cfg(test)]
    pub(crate) fn new_test_with_projects(projects: Vec<(String, PathBuf)>) -> Self {
        let db = Db::open_memory().expect("open in-memory test db");
        let (event_tx, _) = broadcast::channel::<WebEvent>(1000);
        let (raw_sse_tx, _) = broadcast::channel::<String>(2000);
        let projects: Vec<WebProject> = projects
            .into_iter()
            .map(|(name, path)| WebProject {
                name,
                path,
                sessions: Vec::new(),
                active_session: None,
                git_branch: String::new(),
            })
            .collect();
        let inner = Arc::new(RwLock::new(Self::build_inner(&db, projects)));
        let (persist_tx, _persist_rx) = mpsc::unbounded_channel();
        Self {
            inner,
            event_tx,
            raw_sse_tx,
            editor_tx: None,
            db,
            persist_tx,
            runner_registry: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn attach_runner_registry_for_test(
        &mut self,
        runner_registry: Arc<crate::runner::RunnerRegistry>,
    ) {
        self.runner_registry = Some(runner_registry);
        self.register_runner_started_hook();
    }

    /// Access the underlying DB handle (tests only).
    #[cfg(test)]
    pub(crate) fn db_for_test(&self) -> &Db {
        &self.db
    }

    /// Subscribe to internal web-state events (e.g. `RoutineUpdated`).
    ///
    /// Used by the TUI to receive push notifications when the web state
    /// changes, without going through HTTP/SSE.
    pub fn subscribe_events(&self) -> broadcast::Receiver<WebEvent> {
        self.event_tx.subscribe()
    }

    /// Attach the editor broadcast channel after `ServerState` construction.
    pub fn set_editor_tx(&mut self, tx: broadcast::Sender<EditorEvent>) {
        self.editor_tx = Some(tx);
    }

    /// Emit a file-changed event on the editor SSE channel (if attached).
    pub(super) fn emit_editor_file_changed(&self, path: &str, source: &str) {
        if let Some(ref tx) = self.editor_tx {
            let _ = tx.send(EditorEvent::FileChanged {
                path: path.to_string(),
                source: source.to_string(),
            });
        }
    }
}
