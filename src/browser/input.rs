//! Synthetic input, dispatched through CDP so pages cannot tell it from a person.
//!
//! Pane clicks and agent `browser_click` calls both land here; nothing in this module
//! knows which one it is serving, which is what keeps the two behaviours identical.

use serde::Deserialize;
use serde_json::json;

use super::cdp::Cdp;

/// The pane forwards raw pointer phases; tools only ever need a full click.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseKind {
    Move,
    Down,
    Up,
}

impl MouseKind {
    const fn cdp_type(self) -> &'static str {
        match self {
            Self::Move => "mouseMoved",
            Self::Down => "mousePressed",
            Self::Up => "mouseReleased",
        }
    }

    const fn button(self) -> &'static str {
        match self {
            Self::Move => "none",
            Self::Down | Self::Up => "left",
        }
    }

    const fn click_count(self) -> u8 {
        match self {
            Self::Move => 0,
            Self::Down | Self::Up => 1,
        }
    }
}

/// A resolved `[ref=eN]`, straight off the page script.
#[derive(Debug, Deserialize)]
pub(super) struct Resolved {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    x: i64,
    #[serde(default)]
    y: i64,
    #[serde(default)]
    tag: String,
    #[serde(default)]
    editable: bool,
    #[serde(default)]
    select: bool,
}

/// A ref that resolved successfully. Constructing this is the only way past the error
/// arm, so a caller cannot act on coordinates the page never gave us.
#[derive(Debug)]
pub(super) struct Target {
    pub x: i64,
    pub y: i64,
    pub tag: String,
    pub editable: bool,
    #[allow(dead_code)]
    pub select: bool,
}

impl Resolved {
    pub(super) fn into_target(self) -> anyhow::Result<Target> {
        if let Some(error) = self.error {
            return Err(anyhow::anyhow!(error));
        }
        Ok(Target {
            x: self.x,
            y: self.y,
            tag: self.tag,
            editable: self.editable,
            select: self.select,
        })
    }
}

pub(super) async fn mouse(
    cdp: &Cdp,
    session: &str,
    kind: MouseKind,
    x: i64,
    y: i64,
) -> anyhow::Result<()> {
    cdp.call_on(
        session,
        "Input.dispatchMouseEvent",
        json!({
            "type": kind.cdp_type(),
            "x": x,
            "y": y,
            "button": kind.button(),
            "clickCount": kind.click_count(),
        }),
    )
    .await
    .map(drop)
}

/// Move, press, release — the hover step matters on menus that only reveal a target on
/// pointer entry.
pub(super) async fn click(cdp: &Cdp, session: &str, x: i64, y: i64) -> anyhow::Result<()> {
    mouse(cdp, session, MouseKind::Move, x, y).await?;
    mouse(cdp, session, MouseKind::Down, x, y).await?;
    mouse(cdp, session, MouseKind::Up, x, y).await
}

pub(super) async fn scroll(
    cdp: &Cdp,
    session: &str,
    x: i64,
    y: i64,
    delta_y: i64,
) -> anyhow::Result<()> {
    cdp.call_on(
        session,
        "Input.dispatchMouseEvent",
        json!({ "type": "mouseWheel", "x": x, "y": y, "deltaX": 0, "deltaY": delta_y }),
    )
    .await
    .map(drop)
}

/// Bulk text. `Input.insertText` skips per-character key events, which is both faster and
/// closer to a paste — the thing most fields actually handle well.
pub(super) async fn insert_text(cdp: &Cdp, session: &str, text: &str) -> anyhow::Result<()> {
    cdp.call_on(session, "Input.insertText", json!({ "text": text }))
        .await
        .map(drop)
}

/// Clear a focused field without assuming its type: select-all then type replaces.
pub(super) async fn select_all(cdp: &Cdp, session: &str) -> anyhow::Result<()> {
    let key = Key::named("a").with_modifiers(MOD_CTRL);
    dispatch_key(cdp, session, &key, "keyDown").await?;
    dispatch_key(cdp, session, &key, "keyUp").await
}

/// Press a chord such as `Enter`, `Escape`, or `Control+A`.
pub(super) async fn press(cdp: &Cdp, session: &str, chord: &str) -> anyhow::Result<()> {
    let key = Key::parse(chord)?;
    dispatch_key(cdp, session, &key, "keyDown").await?;
    dispatch_key(cdp, session, &key, "keyUp").await
}

const MOD_ALT: i64 = 1;
const MOD_CTRL: i64 = 2;
const MOD_META: i64 = 4;
const MOD_SHIFT: i64 = 8;

/// A chord, already split into the four fields CDP wants.
struct Key {
    key: String,
    code: String,
    virtual_code: i64,
    text: Option<String>,
    modifiers: i64,
}

impl Key {
    fn named(single: &str) -> Self {
        let upper = single.to_ascii_uppercase();
        Self {
            code: format!("Key{upper}"),
            virtual_code: upper.bytes().next().unwrap_or(b'A') as i64,
            key: single.to_owned(),
            text: Some(single.to_owned()),
            modifiers: 0,
        }
    }

    /// Apply modifiers. A modified key produces no text — sending one would make Ctrl+A
    /// insert an "a" alongside selecting everything.
    fn with_modifiers(mut self, modifiers: i64) -> Self {
        self.modifiers = modifiers;
        if modifiers != 0 {
            self.text = None;
        }
        self
    }

    fn parse(chord: &str) -> anyhow::Result<Self> {
        let mut modifiers = 0;
        let mut parts = chord.split('+').peekable();
        let mut base = "";
        while let Some(part) = parts.next() {
            if parts.peek().is_none() {
                base = part;
                break;
            }
            modifiers |= match part.trim().to_ascii_lowercase().as_str() {
                "ctrl" | "control" => MOD_CTRL,
                "shift" => MOD_SHIFT,
                "alt" | "option" => MOD_ALT,
                "meta" | "cmd" | "command" => MOD_META,
                other => return Err(anyhow::anyhow!("unknown modifier `{other}`")),
            };
        }

        let base = base.trim();
        if base.is_empty() {
            return Err(anyhow::anyhow!("empty key in `{chord}`"));
        }

        let key = match NAMED.iter().find(|(name, ..)| name.eq_ignore_ascii_case(base)) {
            Some(&(name, code, virtual_code, text)) => Self {
                key: name.to_owned(),
                code: code.to_owned(),
                virtual_code,
                text: text.map(str::to_owned),
                modifiers: 0,
            },
            None if base.chars().count() == 1 => Self::named(base),
            None => return Err(anyhow::anyhow!("unknown key `{base}`")),
        };
        Ok(key.with_modifiers(modifiers))
    }
}

/// `(key, code, windowsVirtualKeyCode, text)` for the keys worth naming. `text` is what
/// separates a key that inserts a character from one that only fires an event.
const NAMED: [(&str, &str, i64, Option<&str>); 15] = [
    ("Enter", "Enter", 13, Some("\r")),
    ("Tab", "Tab", 9, Some("\t")),
    ("Escape", "Escape", 27, None),
    ("Backspace", "Backspace", 8, None),
    ("Delete", "Delete", 46, None),
    ("ArrowUp", "ArrowUp", 38, None),
    ("ArrowDown", "ArrowDown", 40, None),
    ("ArrowLeft", "ArrowLeft", 37, None),
    ("ArrowRight", "ArrowRight", 39, None),
    ("Home", "Home", 36, None),
    ("End", "End", 35, None),
    ("PageUp", "PageUp", 33, None),
    ("PageDown", "PageDown", 34, None),
    (" ", "Space", 32, Some(" ")),
    ("Space", "Space", 32, Some(" ")),
];

async fn dispatch_key(cdp: &Cdp, session: &str, key: &Key, phase: &str) -> anyhow::Result<()> {
    let mut params = json!({
        "type": phase,
        "key": key.key,
        "code": key.code,
        "windowsVirtualKeyCode": key.virtual_code,
        "nativeVirtualKeyCode": key.virtual_code,
        "modifiers": key.modifiers,
    });

    // keyDown carries text — that is what makes it produce a character. The key must be
    // *absent* rather than null for a key that produces none: CDP validates the type and
    // rejects the whole call on a null.
    if let (Some(text), Some(object)) = (
        (phase == "keyDown").then_some(key.text.as_deref()).flatten(),
        params.as_object_mut(),
    ) {
        object.insert("text".into(), json!(text));
    }

    cdp.call_on(session, "Input.dispatchKeyEvent", params)
        .await
        .map(drop)
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod input_tests;
