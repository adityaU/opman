//! `@test_blockkit` command — Tests A through D.

use super::super::api::{
    post_message, post_message_with_blocks, start_stream, stop_stream, update_message_blocks,
};

/// `@test_blockkit` — Run a battery of tests to determine which Block Kit /
/// markdown approaches work with Slack's streaming API.
///
/// Runs 7 tests sequentially, each starting a short-lived stream:
///   A. Raw markdown table (no code-fence wrapping) via `markdown_text` chunk
///   B. `markdown` chunk type (instead of `markdown_text`)
///   C. Stream → `chat.update` with Block Kit `markdown` block
///   D. `chat.postMessage` with `markdown` block
///   E-G. See test_blockkit_ext module.
pub(super) async fn do_test_blockkit_command(channel: &str, thread_ts: &str, bot_token: &str) {
    let client = reqwest::Client::new();

    let _ = post_message(
        &client,
        bot_token,
        channel,
        ":test_tube: Starting Block Kit test battery…",
        Some(thread_ts),
    )
    .await;

    // Sample markdown with a table, code block, bold, italic, link, list
    let sample_md = sample_markdown();

    // ── Test A: Raw markdown table via streaming `markdown_text` ────────
    {
        let label =
            "**Test A**: Raw markdown table in `markdown_text` chunk (no code-fence wrapping)";
        let text = format!("{}\n\n{}", label, sample_md);

        match start_stream(
            &client,
            bot_token,
            channel,
            thread_ts,
            Some(&text),
            None,
            None,
        )
        .await
        {
            Ok(ts) => {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                let _ = stop_stream(&client, bot_token, channel, &ts).await;
                let _ = post_message(
                    &client,
                    bot_token,
                    channel,
                    ":white_check_mark: Test A stream completed (check rendering above)",
                    Some(thread_ts),
                )
                .await;
            }
            Err(e) => {
                let _ = post_message(
                    &client,
                    bot_token,
                    channel,
                    &format!(":x: Test A failed: {}", e),
                    Some(thread_ts),
                )
                .await;
            }
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // ── Test B: `markdown` chunk type (instead of `markdown_text`) ──────
    {
        let label = "**Test B**: `markdown` chunk type in streaming API";
        let text = format!("{}\n\n{}", label, sample_md);

        // Build a chunk with type "markdown" instead of "markdown_text"
        let md_chunk = serde_json::json!({
            "type": "markdown",
            "text": text,
        });

        match start_stream(
            &client,
            bot_token,
            channel,
            thread_ts,
            None,
            Some(&[md_chunk]),
            None,
        )
        .await
        {
            Ok(ts) => {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                let _ = stop_stream(&client, bot_token, channel, &ts).await;
                let _ = post_message(
                    &client,
                    bot_token,
                    channel,
                    ":white_check_mark: Test B stream completed (check rendering above)",
                    Some(thread_ts),
                )
                .await;
            }
            Err(e) => {
                let _ = post_message(
                    &client,
                    bot_token,
                    channel,
                    &format!(":x: Test B failed: {}", e),
                    Some(thread_ts),
                )
                .await;
            }
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // ── Test C: Stream then chat.update with Block Kit `markdown` block ─
    {
        let label = "**Test C**: Stream → chat.update with `markdown` block";

        match start_stream(
            &client,
            bot_token,
            channel,
            thread_ts,
            Some(label),
            None,
            None,
        )
        .await
        {
            Ok(ts) => {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                let _ = stop_stream(&client, bot_token, channel, &ts).await;
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                // Now update the stopped stream message with a markdown block
                let md_block = serde_json::json!({
                    "type": "markdown",
                    "text": format!("{}\n\n{}", label, sample_md),
                });
                let result = update_message_blocks(
                    &client,
                    bot_token,
                    channel,
                    &ts,
                    "Block Kit test C fallback",
                    &[md_block],
                    None,
                )
                .await;
                match result {
                    Ok(()) => {
                        let _ = post_message(
                            &client,
                            bot_token,
                            channel,
                            ":white_check_mark: Test C update completed (check rendering above)",
                            Some(thread_ts),
                        )
                        .await;
                    }
                    Err(e) => {
                        let _ = post_message(
                            &client,
                            bot_token,
                            channel,
                            &format!(":x: Test C update failed: {}", e),
                            Some(thread_ts),
                        )
                        .await;
                    }
                }
            }
            Err(e) => {
                let _ = post_message(
                    &client,
                    bot_token,
                    channel,
                    &format!(":x: Test C stream failed: {}", e),
                    Some(thread_ts),
                )
                .await;
            }
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // ── Test D: chat.postMessage with `markdown` block ──────────────────
    {
        let label = "**Test D**: `chat.postMessage` with `markdown` block";
        let md_block = serde_json::json!({
            "type": "markdown",
            "text": format!("{}\n\n{}", label, sample_md),
        });

        match post_message_with_blocks(
            &client,
            bot_token,
            channel,
            "Block Kit test D fallback",
            &[md_block],
            None,
            Some(thread_ts),
        )
        .await
        {
            Ok(_ts) => {
                let _ = post_message(
                    &client,
                    bot_token,
                    channel,
                    ":white_check_mark: Test D posted (check rendering above)",
                    Some(thread_ts),
                )
                .await;
            }
            Err(e) => {
                let _ = post_message(
                    &client,
                    bot_token,
                    channel,
                    &format!(":x: Test D failed: {}", e),
                    Some(thread_ts),
                )
                .await;
            }
        }
    }

    // Continue with tests E-G.
    super::test_blockkit_ext::do_test_blockkit_ext(&client, channel, thread_ts, bot_token).await;
}

/// Shared sample markdown used across test cases.
pub(super) fn sample_markdown() -> &'static str {
    concat!(
        "## Test Results\n\n",
        "Here is a **bold** word, an *italic* word, and `inline code`.\n\n",
        "| Feature | Status | Notes |\n",
        "|---------|--------|-------|\n",
        "| Tables  | :white_check_mark: | Native rendering |\n",
        "| Code    | :white_check_mark: | Fenced blocks |\n",
        "| Links   | :white_check_mark: | [Slack](https://slack.com) |\n\n",
        "```rust\nfn main() {\n    println!(\"Hello, Block Kit!\");\n}\n```\n\n",
        "- [ ] Todo item one\n",
        "- [x] Todo item done\n",
        "- [ ] Todo item three\n",
    )
}
