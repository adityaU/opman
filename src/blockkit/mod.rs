//! Markdown → Slack Block Kit conversion.
//!
//! Converts standard Markdown text into Slack Block Kit blocks.  The entire
//! response is emitted as a single `markdown` block (for syntax highlighting
//! in fenced code blocks), with tables extracted as native `table` blocks
//! sent via the `attachments` field.

mod assemble;
#[allow(dead_code)]
mod inline;
mod parse;
mod table;

#[allow(unused_imports)]
pub use parse::MdSegment;
pub use table::{split_around_tables, MdTextSegment};

use assemble::assemble_blocks;
use parse::parse_block_segments;

/// Result of converting markdown to Slack Block Kit.
///
/// Slack’s `table` block must be sent in the `attachments` field (one per
/// message), while all other blocks go in the top-level `blocks` field.
pub struct BlockKitResult {
    /// Regular blocks (markdown, table, etc.) for the `blocks` field.
    pub blocks: Vec<serde_json::Value>,
    /// Table blocks for the `attachments` field.  Slack allows only one table
    /// per message; if there are multiple, only the first is used and the rest
    /// are silently dropped.
    pub table_blocks: Vec<serde_json::Value>,
}

/// Convert markdown text to Slack Block Kit blocks.
///
/// Returns a `BlockKitResult` separating regular blocks (for the `blocks`
/// field) from table blocks (for the `attachments` field).
pub fn markdown_to_blocks(md: &str) -> BlockKitResult {
    let segments = parse_block_segments(md);
    let all_blocks = assemble_blocks(&segments);

    let mut blocks = Vec::new();
    let mut table_blocks = Vec::new();

    for block in all_blocks {
        if block.get("type").and_then(|t| t.as_str()) == Some("table") {
            table_blocks.push(block);
        } else {
            blocks.push(block);
        }
    }

    BlockKitResult {
        blocks,
        table_blocks,
    }
}

// ── Shared types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum TableAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug)]
pub(crate) struct ListItem {
    pub text: String,
    pub indent: usize,
}

#[derive(Debug)]
pub(crate) struct TodoItem {
    pub text: String,
    pub checked: bool,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(crate) enum InlineStyle {
    Bold,
    Italic,
    Code,
    Strike,
    BoldItalic,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_markdown_block_output() {
        let md = r#"# Hello World Examples

Here are hello world programs in three languages:

## Python

```python
def main():
    print("Hello, World!")

if __name__ == "__main__":
    main()
```

## Rust

```rust
fn main() {
    println!("Hello, World!");
}
```

## JavaScript

```javascript
function main() {
    console.log("Hello, World!");
}

main();
```

That's it! Each program prints "Hello, World!" to the console."#;

        let result = markdown_to_blocks(md);

        println!("\n=== BLOCKS ({}) ===", result.blocks.len());
        for (i, block) in result.blocks.iter().enumerate() {
            println!("--- Block {} ---", i);
            println!("{}", serde_json::to_string_pretty(block).unwrap());
        }

        println!("\n=== TABLE BLOCKS ({}) ===", result.table_blocks.len());
        for (i, block) in result.table_blocks.iter().enumerate() {
            println!("--- Table Block {} ---", i);
            println!("{}", serde_json::to_string_pretty(block).unwrap());
        }
    }
}
