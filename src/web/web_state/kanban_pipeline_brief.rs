//! Pure helpers for pipeline-mode launches: which lanes become stages, and how
//! a stage's brief is assembled (prompt + previous output + memory). Kept apart
//! from the orchestration impl so each file stays small and independently testable.

use super::super::types::*;

/// Lanes that form the pipeline, in board order from the task's current lane
/// forward, excluding the terminal review lane. A lane is a stage only if it has
/// an agent or a prompt configured (plain holding columns are skipped).
pub(super) fn pipeline_stage_lanes(board: &Board, current_lane: &str) -> Vec<String> {
    let start = board
        .lanes
        .iter()
        .position(|l| l.id == current_lane)
        .unwrap_or(0);
    board.lanes[start..]
        .iter()
        .filter(|l| !l.terminal && (l.agent.is_some() || l.prompt.is_some()))
        .map(|l| l.id.clone())
        .collect()
}

pub(super) fn build_stage_brief(
    task: &Task,
    lane: &Lane,
    index: usize,
    total: usize,
    prev_output: Option<&str>,
) -> String {
    let mut s = format!(
        "You are running ONE STAGE of a multi-stage Kanban pipeline.\n\n\
         TASK: {title}\nSTAGE {n}/{total}: {lane}\nPRIORITY: {prio}\n\n\
         TASK BRIEF:\n{desc}\n\n\
         STAGE INSTRUCTIONS:\n{prompt}\n\n",
        title = task.title,
        n = index + 1,
        total = total,
        lane = lane.name,
        prio = task.priority,
        desc = if task.description.is_empty() {
            "(no description)"
        } else {
            &task.description
        },
        prompt = lane
            .prompt
            .as_deref()
            .filter(|p| !p.trim().is_empty())
            .unwrap_or(
                "Advance the task for this stage and produce a clear handoff for the next stage."
            ),
    );
    if let Some(prev) = prev_output {
        s.push_str(&format!(
            "OUTPUT FROM THE PREVIOUS STAGE — build on this:\n---\n{}\n---\n\n",
            truncate(prev, 8000)
        ));
    }
    s.push_str(&format!(
        "Use the `kanban` MCP tools to log progress: kanban_add_note(task_id=\"{id}\", body=…).\n\
         Do NOT move lanes yourself — the pipeline advances lanes automatically when you finish.\n\
         When the stage is done, end your turn with a concise summary of what you produced; that \
         summary is handed to the next stage as its input. Begin now.",
        id = task.id,
    ));
    s
}

/// Open a pipeline stage with the standing session instructions in place, using
/// the one formatter every session-opening path shares.
pub(super) fn inject_memory(brief: &str, memory: &[PersonalMemoryItem]) -> String {
    crate::web::session_instructions::wrap(brief, memory)
}

pub(super) fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max).collect();
    format!("{truncated}\n…[truncated]")
}

#[cfg(test)]
#[path = "kanban_pipeline_brief_tests.rs"]
mod kanban_pipeline_brief_tests;
