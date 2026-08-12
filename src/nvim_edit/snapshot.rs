//! One round-trip that asks Neovim everything the editor paints.
//!
//! Cursor, mode, visual anchor, search highlight and window layout used to be
//! four separate RPCs plus a polling loop that tried to guess when Neovim had
//! settled. Neovim says when it has settled — it emits `flush` — so this runs
//! once per flush and asks for all of it at once.

use rmpv::Value;

use crate::nvim_ui::stream::wire::{BufferEntry, Layout, NvimMode};

pub(super) const SNAPSHOT_LUA: &str = "local cursor = vim.api.nvim_win_get_cursor(0)
local anchor = vim.fn.getpos('v')
local buffers = {}
for _, id in ipairs(vim.api.nvim_list_bufs()) do
  if vim.api.nvim_buf_is_loaded(id) and vim.bo[id].buflisted then
    buffers[#buffers + 1] = {
      name = vim.api.nvim_buf_get_name(id),
      modified = vim.bo[id].modified,
      current = id == vim.api.nvim_get_current_buf(),
    }
  end
end
return {
  mode = vim.api.nvim_get_mode().mode,
  row = cursor[1],
  col = cursor[2],
  anchor_row = anchor[2],
  anchor_col = anchor[3],
  search = (vim.v.hlsearch == 1) and vim.fn.getreg('/') or '',
  tabpages = #vim.api.nvim_list_tabpages(),
  windows = #vim.api.nvim_tabpage_list_wins(0),
  buffers = buffers,
}";

/// Everything one flush reports, already in opman's own vocabulary.
pub(super) struct Snapshot {
    pub(super) mode: NvimMode,
    pub(super) row: usize,
    pub(super) byte: usize,
    pub(super) anchor: Option<(usize, usize)>,
    pub(super) search: Option<String>,
    pub(super) layout: Layout,
}

impl Snapshot {
    pub(super) fn parse(value: &Value, prefix: Option<&str>) -> Option<Self> {
        let mode = NvimMode::try_from(field(value, "mode")?.as_str()?).ok()?;
        let row = number(value, "row")?;
        let anchor_row = number(value, "anchor_row").unwrap_or(0);
        let anchor_col = number(value, "anchor_col").unwrap_or(0);
        let search = field(value, "search")
            .and_then(Value::as_str)
            .filter(|pattern| !pattern.is_empty())
            .map(std::borrow::ToOwned::to_owned);
        Some(Self {
            mode,
            row: row.saturating_sub(1),
            byte: number(value, "col").unwrap_or(0),
            anchor: (anchor_row > 0 && anchor_col > 0)
                .then(|| (anchor_row.saturating_sub(1), anchor_col.saturating_sub(1))),
            search,
            layout: Layout {
                tabpages: number(value, "tabpages").unwrap_or(1) as u32,
                windows: number(value, "windows").unwrap_or(1) as u32,
                buffers: field(value, "buffers")
                    .and_then(Value::as_array)
                    .map(|entries| entries.iter().map(|entry| buffer(entry, prefix)).collect())
                    .unwrap_or_default(),
            },
        })
    }
}

fn buffer(value: &Value, prefix: Option<&str>) -> BufferEntry {
    let name = field(value, "name").and_then(Value::as_str).unwrap_or("");
    BufferEntry {
        name: display_name(name, prefix),
        modified: field(value, "modified")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        current: field(value, "current")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

fn display_name(name: &str, prefix: Option<&str>) -> String {
    if name.is_empty() {
        return "[No Name]".into();
    }
    prefix
        .and_then(|prefix| name.strip_prefix(prefix))
        .unwrap_or(name)
        .to_owned()
}

fn number(value: &Value, key: &str) -> Option<usize> {
    let found = field(value, key)?;
    found
        .as_u64()
        .or_else(|| found.as_i64().and_then(|n| u64::try_from(n).ok()))
        .and_then(|n| usize::try_from(n).ok())
}

fn field<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value
        .as_map()?
        .iter()
        .find(|(name, _)| name.as_str() == Some(key))
        .map(|(_, value)| value)
}

#[cfg(test)]
mod tests {
    use super::Snapshot;
    use crate::nvim_ui::stream::wire::NvimMode;
    use rmpv::Value;

    fn map(pairs: Vec<(&str, Value)>) -> Value {
        Value::Map(
            pairs
                .into_iter()
                .map(|(key, value)| (Value::from(key), value))
                .collect(),
        )
    }

    #[test]
    fn one_flush_reports_cursor_mode_search_and_layout() {
        let value = map(vec![
            ("mode", "v".into()),
            ("row", 3.into()),
            ("col", 4.into()),
            ("anchor_row", 2.into()),
            ("anchor_col", 1.into()),
            ("search", "needle".into()),
            ("tabpages", 2.into()),
            ("windows", 3.into()),
            (
                "buffers",
                Value::Array(vec![map(vec![
                    ("name", "/work/main.txt".into()),
                    ("modified", true.into()),
                    ("current", true.into()),
                ])]),
            ),
        ]);
        let parsed = Snapshot::parse(&value, Some("/work/")).expect("snapshot parses");
        assert_eq!(parsed.mode, NvimMode::Visual);
        assert_eq!((parsed.row, parsed.byte), (2, 4));
        assert_eq!(parsed.anchor, Some((1, 0)));
        assert_eq!(parsed.search.as_deref(), Some("needle"));
        assert_eq!(parsed.layout.tabpages, 2);
        assert_eq!(parsed.layout.buffers[0].name, "main.txt");
    }

    #[test]
    fn an_empty_search_register_means_no_highlight() {
        let value = map(vec![
            ("mode", "n".into()),
            ("row", 1.into()),
            ("col", 0.into()),
            ("search", "".into()),
        ]);
        let parsed = Snapshot::parse(&value, None).expect("snapshot parses");
        assert_eq!(parsed.search, None);
        assert_eq!(parsed.anchor, None);
    }
}
