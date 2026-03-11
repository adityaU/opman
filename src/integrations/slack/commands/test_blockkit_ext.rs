//! `@test_blockkit` command — Tests E through G + completion message.

use super::super::api::{post_message, post_message_with_blocks};

/// Run tests E, F, G and the completion message.
/// Called from `test_blockkit::do_test_blockkit_command`.
pub(super) async fn do_test_blockkit_ext(
    client: &reqwest::Client,
    channel: &str,
    thread_ts: &str,
    bot_token: &str,
) {
    // ── Test E: chat.postMessage with rich_text block (table) ───────────
    {
        let rich_text_block = serde_json::json!({
            "type": "rich_text",
            "elements": [
                {
                    "type": "rich_text_section",
                    "elements": [
                        {
                            "type": "text",
                            "text": "Test E: ",
                            "style": { "bold": true }
                        },
                        {
                            "type": "text",
                            "text": "rich_text block with styled text, code, and quote"
                        }
                    ]
                },
                {
                    "type": "rich_text_preformatted",
                    "elements": [
                        {
                            "type": "text",
                            "text": "fn main() {\n    println!(\"Hello from rich_text!\");\n}"
                        }
                    ]
                },
                {
                    "type": "rich_text_quote",
                    "elements": [
                        {
                            "type": "text",
                            "text": "This is a blockquoted user message"
                        }
                    ]
                },
                {
                    "type": "rich_text_list",
                    "style": "bullet",
                    "elements": [
                        {
                            "type": "rich_text_section",
                            "elements": [
                                { "type": "text", "text": "Bullet item one" }
                            ]
                        },
                        {
                            "type": "rich_text_section",
                            "elements": [
                                { "type": "text", "text": "Bullet item two (", "style": { "bold": true } },
                                { "type": "text", "text": "bold", "style": { "bold": true } },
                                { "type": "text", "text": ")" }
                            ]
                        }
                    ]
                }
            ]
        });

        match post_message_with_blocks(
            client,
            bot_token,
            channel,
            "Block Kit test E fallback",
            &[rich_text_block],
            None,
            Some(thread_ts),
        )
        .await
        {
            Ok(_ts) => {
                let _ = post_message(
                    client,
                    bot_token,
                    channel,
                    ":white_check_mark: Test E posted (check rendering above)",
                    Some(thread_ts),
                )
                .await;
            }
            Err(e) => {
                let _ = post_message(
                    client,
                    bot_token,
                    channel,
                    &format!(":x: Test E failed: {}", e),
                    Some(thread_ts),
                )
                .await;
            }
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // ── Test F: chat.postMessage with `table` block (structured JSON) ───
    {
        let table_block = serde_json::json!({
            "type": "table",
            "columns": [
                { "id": "col_feature", "header": "Feature",  "width": 3 },
                { "id": "col_status",  "header": "Status",   "width": 2 },
                { "id": "col_notes",   "header": "Notes",    "width": 5 },
            ],
            "rows": [
                {
                    "id": "row_1",
                    "cells": {
                        "col_feature": { "type": "plain_text", "text": "Tables" },
                        "col_status":  { "type": "plain_text", "text": ":white_check_mark:" },
                        "col_notes":   { "type": "plain_text", "text": "Native rendering via table block" },
                    }
                },
                {
                    "id": "row_2",
                    "cells": {
                        "col_feature": { "type": "plain_text", "text": "Code" },
                        "col_status":  { "type": "plain_text", "text": ":white_check_mark:" },
                        "col_notes":   { "type": "plain_text", "text": "Fenced code blocks" },
                    }
                },
                {
                    "id": "row_3",
                    "cells": {
                        "col_feature": { "type": "plain_text", "text": "Links" },
                        "col_status":  { "type": "plain_text", "text": ":white_check_mark:" },
                        "col_notes":   { "type": "mrkdwn", "text": "<https://slack.com|Slack>" },
                    }
                },
            ]
        });

        match post_message_with_blocks(
            client,
            bot_token,
            channel,
            "Block Kit test F fallback — table block",
            &[table_block],
            None,
            Some(thread_ts),
        )
        .await
        {
            Ok(_ts) => {
                let _ = post_message(
                    client,
                    bot_token,
                    channel,
                    ":white_check_mark: Test F posted (table block — check rendering above)",
                    Some(thread_ts),
                )
                .await;
            }
            Err(e) => {
                let _ = post_message(
                    client,
                    bot_token,
                    channel,
                    &format!(":x: Test F failed: {}", e),
                    Some(thread_ts),
                )
                .await;
            }
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // ── Test G: table block with alternate schema (rows as arrays) ──────
    // The docs showed two possible formats — try the array-of-arrays style
    {
        let table_block = serde_json::json!({
            "type": "table",
            "column_settings": [
                { "is_wrapped": true },
                { "align": "center" },
                { "align": "left", "is_wrapped": true },
            ],
            "rows": [
                [
                    { "type": "raw_text", "text": "Feature" },
                    { "type": "raw_text", "text": "Status" },
                    { "type": "raw_text", "text": "Notes" },
                ],
                [
                    { "type": "raw_text", "text": "Tables" },
                    { "type": "raw_text", "text": "\u{2705}" },
                    { "type": "raw_text", "text": "Native table block" },
                ],
                [
                    { "type": "raw_text", "text": "Code" },
                    { "type": "raw_text", "text": "\u{2705}" },
                    { "type": "raw_text", "text": "Fenced code blocks" },
                ],
                [
                    { "type": "raw_text", "text": "Links" },
                    { "type": "raw_text", "text": "\u{2705}" },
                    {
                        "type": "rich_text",
                        "elements": [{
                            "type": "rich_text_section",
                            "elements": [{
                                "type": "link",
                                "url": "https://slack.com",
                                "text": "Slack"
                            }]
                        }]
                    },
                ],
            ]
        });

        match post_message_with_blocks(
            client,
            bot_token,
            channel,
            "Block Kit test G fallback — table block (array rows)",
            &[table_block],
            None,
            Some(thread_ts),
        )
        .await
        {
            Ok(_ts) => {
                let _ = post_message(
                    client,
                    bot_token,
                    channel,
                    ":white_check_mark: Test G posted (table block array format — check rendering above)",
                    Some(thread_ts),
                )
                .await;
            }
            Err(e) => {
                let _ = post_message(
                    client,
                    bot_token,
                    channel,
                    &format!(":x: Test G failed: {}", e),
                    Some(thread_ts),
                )
                .await;
            }
        }
    }

    let _ = post_message(
        client,
        bot_token,
        channel,
        ":checkered_flag: Block Kit test battery complete. Check each test message above for rendering quality.",
        Some(thread_ts),
    )
    .await;
}
