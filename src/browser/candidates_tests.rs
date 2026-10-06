use super::*;

/// Position of the first entry containing `needle`, so orderings can be asserted without
/// repeating every path.
fn rank(list: &[&str], needle: &str) -> usize {
    list.iter()
        .position(|entry| entry.contains(needle))
        .unwrap_or_else(|| panic!("{needle} is not a candidate in {list:?}"))
}

fn assert_chrome_edge_brave_chromium(list: &[&str], [chrome, edge, brave, chromium]: [&str; 4]) {
    let order = [
        rank(list, chrome),
        rank(list, edge),
        rank(list, brave),
        rank(list, chromium),
    ];
    assert!(
        order.windows(2).all(|pair| pair[0] < pair[1]),
        "expected Chrome → Edge → Brave → Chromium, got {order:?} in {list:?}"
    );
}

#[test]
fn device_mode_prefers_chrome_then_edge_then_brave_then_chromium_on_path() {
    assert_chrome_edge_brave_chromium(
        path_names(BrowserMode::Device),
        ["google-chrome", "microsoft-edge", "brave", "chromium"],
    );
}

#[test]
fn device_mode_ordering_holds_on_macos() {
    assert_chrome_edge_brave_chromium(
        mac_bundles(BrowserMode::Device),
        ["Google Chrome", "Microsoft Edge", "Brave", "Chromium"],
    );
}

#[test]
fn device_mode_ordering_holds_on_windows() {
    assert_chrome_edge_brave_chromium(
        windows_suffixes(BrowserMode::Device),
        ["chrome.exe", "msedge.exe", "brave.exe", "Chromium\\"],
    );
}

#[test]
fn server_mode_still_prefers_plain_chromium() {
    assert_eq!(path_names(BrowserMode::Server).first(), Some(&"chromium"));
}

#[test]
fn edge_is_a_candidate_in_every_list() {
    for mode in [BrowserMode::Server, BrowserMode::Device] {
        assert!(path_names(mode)
            .iter()
            .any(|n| n.contains("microsoft-edge")));
        assert!(mac_bundles(mode)
            .iter()
            .any(|n| n.contains("Microsoft Edge")));
        assert!(windows_suffixes(mode)
            .iter()
            .any(|n| n.ends_with("msedge.exe")));
    }
}

#[test]
fn expansion_is_browser_major() {
    let roots = [PathBuf::from("/a"), PathBuf::from("/b")];
    let expanded = expand(&roots, &["chrome", "edge"]);
    assert_eq!(
        expanded,
        vec![
            PathBuf::from("/a/chrome"),
            PathBuf::from("/b/chrome"),
            PathBuf::from("/a/edge"),
            PathBuf::from("/b/edge"),
        ]
    );
}
