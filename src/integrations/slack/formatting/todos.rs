//! Todo-list rendering helpers.

/// Render a list of `TodoItem`s as plain markdown checklist text.
///
/// Each item is rendered as a standard markdown checkbox:
/// - completed  → `- [x]`
/// - in_progress → `- [-]`
/// - cancelled  → `- [~]`
/// - pending    → `- [ ]`
///
/// High-priority items are suffixed with `[HIGH]`.
pub fn render_todos_mrkdwn(todos: &[crate::app::TodoItem]) -> String {
    if todos.is_empty() {
        return "*Todo List*\n_No items yet._".to_string();
    }

    let mut lines = Vec::with_capacity(todos.len() + 2);
    lines.push("*Todo List*".to_string());
    lines.push(String::new());

    for item in todos {
        let checkbox = match item.status.as_str() {
            "completed" => "- [x]",
            "in_progress" => "- [-]",
            "cancelled" => "- [~]",
            _ => "- [ ]", // pending / unknown
        };

        let priority_suffix = match item.priority.as_str() {
            "high" => "  `[HIGH]`",
            _ => "",
        };

        let content = if item.status == "completed" || item.status == "cancelled" {
            format!("~{}~", item.content) // strikethrough
        } else {
            item.content.clone()
        };

        lines.push(format!("{} {}{}", checkbox, content, priority_suffix));
    }

    // Add a summary line.
    let done = todos.iter().filter(|t| t.status == "completed").count();
    let total = todos.len();
    lines.push(format!("\n_{}/{} completed_", done, total));
    lines.join("\n")
}
