/// Tests for standalone helper functions (diff, terminal buffer, session ownership).
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::app::helpers::{diff_snapshot_lines, parse_unified_diff};
    use crate::app::SessionInfo;

    fn make_session(id: &str) -> SessionInfo {
        SessionInfo {
            id: id.to_string(),
            ..Default::default()
        }
    }

    /// Mirrors the SessionsFetched ownership recording from handle_background_event.
    fn record_ownership(
        sessions: &[SessionInfo],
        project_idx: usize,
        ownership: &mut HashMap<String, usize>,
    ) {
        for s in sessions {
            ownership.insert(s.id.clone(), project_idx);
        }
    }

    /// Mirrors the SseSessionCreated/Updated ownership guard.
    fn is_owned_by_other(
        session_id: &str,
        project_idx: usize,
        ownership: &HashMap<String, usize>,
    ) -> bool {
        if let Some(&owner) = ownership.get(session_id) {
            return owner != project_idx;
        }
        false
    }

    #[test]
    fn test_sessions_fetched_records_ownership() {
        let mut ownership = HashMap::new();
        let sessions = vec![make_session("s1"), make_session("s2"), make_session("s3")];
        record_ownership(&sessions, 0, &mut ownership);

        assert_eq!(ownership.len(), 3);
        assert_eq!(ownership["s1"], 0);
        assert_eq!(ownership["s2"], 0);
        assert_eq!(ownership["s3"], 0);
    }

    #[test]
    fn test_sessions_fetched_overwrites_stale_ownership() {
        let mut ownership = HashMap::new();
        ownership.insert("s1".to_string(), 0);
        let sessions = vec![make_session("s1")];
        record_ownership(&sessions, 1, &mut ownership);
        assert_eq!(ownership["s1"], 1);
    }

    #[test]
    fn test_sse_session_created_skips_if_owned_by_other() {
        let mut ownership = HashMap::new();
        ownership.insert("s1".to_string(), 0);

        assert!(is_owned_by_other("s1", 1, &ownership));
        assert!(!is_owned_by_other("s1", 0, &ownership));
    }

    #[test]
    fn test_sse_session_created_claims_if_new() {
        let mut ownership = HashMap::new();
        assert!(!is_owned_by_other("s1", 1, &ownership));
        ownership.entry("s1".to_string()).or_insert(1);
        assert_eq!(ownership["s1"], 1);
    }

    #[test]
    fn test_sse_session_updated_skips_if_owned_by_other() {
        let mut ownership = HashMap::new();
        ownership.insert("s1".to_string(), 0);

        assert!(is_owned_by_other("s1", 1, &ownership));
        assert!(!is_owned_by_other("s1", 0, &ownership));
        assert!(!is_owned_by_other("unknown", 0, &ownership));
    }

    #[test]
    fn test_session_deleted_removes_ownership() {
        let mut ownership = HashMap::new();
        ownership.insert("s1".to_string(), 0);
        ownership.remove("s1");
        assert!(!ownership.contains_key("s1"));
    }

    #[test]
    fn test_awaiting_session_overrides_ownership() {
        let mut ownership = HashMap::new();
        ownership.insert("s1".to_string(), 0);
        ownership.insert("s1".to_string(), 1);
        assert_eq!(ownership["s1"], 1);
    }

    #[test]
    fn test_deleted_session_can_be_reclaimed() {
        let mut ownership = HashMap::new();
        ownership.insert("s1".to_string(), 0);
        ownership.remove("s1");

        assert!(!is_owned_by_other("s1", 1, &ownership));
        ownership.entry("s1".to_string()).or_insert(1);
        assert_eq!(ownership["s1"], 1);
    }

    #[test]
    fn test_multiple_projects_independent_sessions() {
        let mut ownership = HashMap::new();

        let p0_sessions = vec![make_session("s1"), make_session("s2")];
        record_ownership(&p0_sessions, 0, &mut ownership);

        let p1_sessions = vec![make_session("s3"), make_session("s4")];
        record_ownership(&p1_sessions, 1, &mut ownership);

        assert_eq!(ownership["s1"], 0);
        assert_eq!(ownership["s2"], 0);
        assert_eq!(ownership["s3"], 1);
        assert_eq!(ownership["s4"], 1);

        assert!(is_owned_by_other("s3", 0, &ownership));
        assert!(is_owned_by_other("s1", 1, &ownership));
    }

    #[test]
    fn test_parse_unified_diff_additions() {
        let diff = "\
diff --git a/src/main.rs b/src/main.rs
index 1234567..abcdef0 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -10,0 +11,3 @@ fn main() {
+    let x = 1;
+    let y = 2;
+    let z = 3;
";
        let (added, deleted) = parse_unified_diff(diff);
        assert_eq!(added, vec![11, 12, 13]);
        assert!(deleted.is_empty());
    }

    #[test]
    fn test_parse_unified_diff_deletions() {
        let diff = "\
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -5,2 +5,0 @@ fn main() {
-    old_line_1();
-    old_line_2();
";
        let (added, deleted) = parse_unified_diff(diff);
        assert!(added.is_empty());
        assert_eq!(deleted, vec![5]);
    }

    #[test]
    fn test_parse_unified_diff_mixed() {
        let diff = "\
@@ -3,2 +3,4 @@ fn foo() {
-    old1();
-    old2();
+    new1();
+    new2();
+    new3();
+    new4();
@@ -20,1 +22,0 @@ fn bar() {
-    removed();
";
        let (added, deleted) = parse_unified_diff(diff);
        assert_eq!(added, vec![3, 4, 5, 6]);
        assert_eq!(deleted, vec![22]);
    }

    #[test]
    fn test_parse_unified_diff_single_line() {
        let diff = "@@ -1 +1 @@\n-old\n+new\n";
        let (added, deleted) = parse_unified_diff(diff);
        assert_eq!(added, vec![1]);
        assert!(deleted.is_empty());
    }

    #[test]
    fn test_snapshot_diff_additions() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nline2\nnew_line\nline3\n";
        let (added, deleted) = diff_snapshot_lines(old, new);
        assert_eq!(added, vec![3]);
        assert!(deleted.is_empty());
    }

    #[test]
    fn test_snapshot_diff_deletions() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nline3\n";
        let (added, deleted) = diff_snapshot_lines(old, new);
        assert!(added.is_empty());
        assert_eq!(deleted, vec![1]);
    }

    #[test]
    fn test_snapshot_diff_mixed() {
        let old = "aaa\nbbb\nccc\n";
        let new = "aaa\nXXX\nccc\nYYY\n";
        let (added, deleted) = diff_snapshot_lines(old, new);
        assert_eq!(added, vec![2, 4]);
        assert_eq!(deleted, vec![1]);
    }

    #[test]
    fn test_snapshot_diff_empty_to_content() {
        let old = "";
        let new = "hello\nworld\n";
        let (added, deleted) = diff_snapshot_lines(old, new);
        assert_eq!(added, vec![1, 2]);
        assert!(deleted.is_empty());
    }

    #[test]
    fn test_snapshot_diff_no_change() {
        let old = "same\ncontent\n";
        let new = "same\ncontent\n";
        let (added, deleted) = diff_snapshot_lines(old, new);
        assert!(added.is_empty());
        assert!(deleted.is_empty());
    }
}
