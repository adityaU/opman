//! Modal state management.
//! Matches React `useModalState.ts` — tracks all modal open/close states.

use leptos::prelude::*;
use std::collections::HashMap;

/// All modal names in the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModalName {
    CommandPalette,
    ModelPicker,
    AgentPicker,
    ThemeSelector,
    Cheatsheet,
    TodoPanel,
    SessionSelector,
    ContextInput,
    Settings,
    Watcher,
    ContextWindow,
    DiffReview,
    SearchBar,
    CrossSearch,
    SplitView,
    SessionGraph,
    SessionDashboard,
    ActivityFeed,
    NotificationPrefs,
    AssistantCenter,
    Inbox,
    Memory,
    Autonomy,
    Routines,
    Delegation,
    Missions,
    WorkspaceManager,
    AddProject,
    SystemMonitor,
    SessionSearch,
}

impl ModalName {
    /// Convert a string name (from commands/React) to a ModalName.
    pub fn from_str_name(name: &str) -> Option<Self> {
        match name {
            "commandPalette" => Some(Self::CommandPalette),
            "modelPicker" => Some(Self::ModelPicker),
            "agentPicker" => Some(Self::AgentPicker),
            "themeSelector" => Some(Self::ThemeSelector),
            "cheatsheet" => Some(Self::Cheatsheet),
            "todoPanel" => Some(Self::TodoPanel),
            "sessionSelector" => Some(Self::SessionSelector),
            "contextInput" => Some(Self::ContextInput),
            "settings" => Some(Self::Settings),
            "watcher" => Some(Self::Watcher),
            "contextWindow" => Some(Self::ContextWindow),
            "diffReview" => Some(Self::DiffReview),
            "searchBar" => Some(Self::SearchBar),
            "crossSearch" => Some(Self::CrossSearch),
            "splitView" => Some(Self::SplitView),
            "sessionGraph" => Some(Self::SessionGraph),
            "sessionDashboard" => Some(Self::SessionDashboard),
            "activityFeed" => Some(Self::ActivityFeed),
            "notificationPrefs" => Some(Self::NotificationPrefs),
            "assistantCenter" => Some(Self::AssistantCenter),
            "inbox" => Some(Self::Inbox),
            "memory" => Some(Self::Memory),
            "autonomy" => Some(Self::Autonomy),
            "routines" => Some(Self::Routines),
            "delegation" => Some(Self::Delegation),
            "missions" => Some(Self::Missions),
            "workspaceManager" => Some(Self::WorkspaceManager),
            "addProject" => Some(Self::AddProject),
            "systemMonitor" => Some(Self::SystemMonitor),
            "sessionSearch" => Some(Self::SessionSearch),
            _ => None,
        }
    }
}

/// Escape-key dismiss priority, highest first.
const ESCAPE_PRIORITY: &[ModalName] = &[
    ModalName::CommandPalette,
    ModalName::ModelPicker,
    ModalName::AgentPicker,
    ModalName::ThemeSelector,
    ModalName::Cheatsheet,
    ModalName::TodoPanel,
    ModalName::SessionSelector,
    ModalName::ContextInput,
    ModalName::Settings,
    ModalName::Watcher,
    ModalName::ContextWindow,
    ModalName::DiffReview,
    ModalName::SearchBar,
    ModalName::CrossSearch,
    ModalName::ActivityFeed,
    ModalName::NotificationPrefs,
    ModalName::AssistantCenter,
    ModalName::Inbox,
    ModalName::Memory,
    ModalName::Autonomy,
    ModalName::Routines,
    ModalName::Delegation,
    ModalName::Missions,
    ModalName::WorkspaceManager,
    ModalName::AddProject,
    ModalName::SystemMonitor,
    ModalName::SessionSearch,
];

/// Full modal state API.
#[derive(Clone, Copy)]
pub struct ModalState {
    modals: ReadSignal<HashMap<ModalName, bool>>,
    set_modals: WriteSignal<HashMap<ModalName, bool>>,
    // Search auxiliary state
    pub search_match_ids: ReadSignal<Vec<String>>,
    pub set_search_match_ids: WriteSignal<Vec<String>>,
    pub active_search_match_id: ReadSignal<Option<String>>,
    pub set_active_search_match_id: WriteSignal<Option<String>>,
    // Split-view auxiliary state
    pub split_view_secondary_id: ReadSignal<Option<String>>,
    pub set_split_view_secondary_id: WriteSignal<Option<String>>,
}

impl ModalState {
    /// Check whether a modal is currently open.
    pub fn is_open(&self, name: ModalName) -> bool {
        self.modals
            .get_untracked()
            .get(&name)
            .copied()
            .unwrap_or(false)
    }

    /// Reactive check — tracks the signal.
    pub fn is_open_tracked(&self, name: ModalName) -> bool {
        self.modals.get().get(&name).copied().unwrap_or(false)
    }

    /// Open a modal by name.
    pub fn open(&self, name: ModalName) {
        self.set_modals.update(|m| {
            m.insert(name, true);
        });
    }

    /// Open a modal by string name (from commands).
    pub fn open_str(&self, name: &str) {
        if let Some(modal) = ModalName::from_str_name(name) {
            self.open(modal);
        }
    }

    /// Close a modal by name.
    pub fn close(&self, name: ModalName) {
        self.set_modals.update(|m| {
            m.insert(name, false);
        });
        self.cleanup_side_effects(name);
    }

    /// Toggle a modal by name.
    pub fn toggle(&self, name: ModalName) {
        let currently_open = self.is_open(name);
        if currently_open {
            self.close(name);
        } else {
            self.open(name);
        }
    }

    /// Close the highest-priority open modal. Returns true if one was closed.
    pub fn close_top_modal(&self) -> bool {
        let current = self.modals.get_untracked();
        for &name in ESCAPE_PRIORITY {
            if current.get(&name).copied().unwrap_or(false) {
                self.close(name);
                return true;
            }
        }
        false
    }

    fn cleanup_side_effects(&self, name: ModalName) {
        match name {
            ModalName::SearchBar => {
                self.set_search_match_ids.set(Vec::new());
                self.set_active_search_match_id.set(None);
            }
            ModalName::SplitView => {
                self.set_split_view_secondary_id.set(None);
            }
            _ => {}
        }
    }
}

/// Create the modal state. Call once at the layout level.
pub fn use_modal_state() -> ModalState {
    let initial: HashMap<ModalName, bool> = HashMap::new();
    let (modals, set_modals) = signal(initial);
    let (search_match_ids, set_search_match_ids) = signal::<Vec<String>>(Vec::new());
    let (active_search_match_id, set_active_search_match_id) = signal::<Option<String>>(None);
    let (split_view_secondary_id, set_split_view_secondary_id) = signal::<Option<String>>(None);

    ModalState {
        modals,
        set_modals,
        search_match_ids,
        set_search_match_ids,
        active_search_match_id,
        set_active_search_match_id,
        split_view_secondary_id,
        set_split_view_secondary_id,
    }
}
