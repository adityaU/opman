//! Reusable UI components.

pub mod icons;
pub mod error_boundary;
pub mod toast;
pub mod app_loader;
pub mod chat_layout;
pub mod chat_main_area;
pub mod chat_sidebar;
pub mod status_bar;
pub mod mobile_dock;
pub mod panel_floating_header;
pub mod prompt_input;
pub mod message_timeline;
pub mod permission_dock;
pub mod question_dock;
pub mod code_block;
pub mod syntax_highlight;
pub mod tool_call;
pub mod subagent_session;
pub mod message_turn;
pub mod slash_command_popover;
pub mod search_bar;

// Modal infrastructure
pub mod modal_overlay;
pub mod modal_layer;

// Phase 6: Core modals
pub mod command_palette;
pub mod model_picker_modal;
pub mod agent_picker_modal;
pub mod theme_selector_modal;
pub mod cheatsheet_modal;
pub mod session_selector_modal;
pub mod context_input_modal;
pub mod settings_modal;
pub mod add_project_modal;
pub mod add_project_entry;

// Phase 7: Feature modals / panels
pub mod todo_panel_modal;
pub mod context_window_panel;
pub mod diff_review_panel;
pub mod cross_session_search_modal;
pub mod split_view;
pub mod session_graph;
pub mod session_dashboard;
pub mod activity_feed;

// Phase 8: Assistant modals
pub mod autonomy_modal;
pub mod notification_prefs_modal;
pub mod auto_open_modal;
pub mod delegation_board_modal;
pub mod memory_modal;
pub mod missions_modal;
pub mod inbox_modal;
pub mod workspace_manager_modal;
pub mod assistant_center_modal;
pub mod system_monitor_modal;
pub mod routines_modal;
pub mod watcher_modal;
pub mod session_search_modal;

// Phase 9: Panels
pub mod terminal_panel;
pub mod code_editor_panel;
pub mod git_panel;

// Debug
pub mod debug_overlay;

// Process Health
pub mod process_health_drawer;
