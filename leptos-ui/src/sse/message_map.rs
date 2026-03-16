//! Message map — efficient ordered message collection, matching React `messageMap.ts`.
//!
//! Uses a HashMap<String, Message> keyed by effective message ID.
//! Provides upsert/merge operations for SSE-driven incremental updates.

use crate::types::core::{Message, MessageInfo, MessagePart};
use std::collections::HashMap;

/// Map of message ID → Message. Used as the authoritative store.
pub type MessageMap = HashMap<String, Message>;

/// Get the effective ID for a message.
pub fn effective_id(info: &MessageInfo) -> String {
    info.message_id
        .clone()
        .or_else(|| info.id.clone())
        .unwrap_or_default()
}

/// Get a message's sort timestamp.
pub fn get_message_time(msg: &Message) -> f64 {
    msg.info.time_f64()
}

/// Convert a message map to a sorted Vec<Message> (oldest first).
pub fn map_to_sorted_array(map: &MessageMap) -> Vec<Message> {
    let mut msgs: Vec<Message> = map.values().cloned().collect();
    msgs.sort_by(|a, b| {
        get_message_time(a)
            .partial_cmp(&get_message_time(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    msgs
}

/// Upsert a message's info into the map. Returns true if the map changed.
pub fn upsert_message_info(map: &mut MessageMap, info_value: &serde_json::Value) -> bool {
    let info: MessageInfo = match serde_json::from_value(info_value.clone()) {
        Ok(i) => i,
        Err(_) => return false,
    };
    let id = effective_id(&info);
    if id.is_empty() {
        return false;
    }

    if let Some(existing) = map.get_mut(&id) {
        // Merge info fields — the new info takes precedence for non-None fields
        merge_info(&mut existing.info, &info);
        true
    } else {
        // Create new message with this info
        map.insert(
            id,
            Message {
                info,
                parts: Vec::new(),
                metadata: None,
            },
        );
        true
    }
}

/// Merge incoming info into existing info (new values overwrite).
fn merge_info(existing: &mut MessageInfo, incoming: &MessageInfo) {
    if incoming.message_id.is_some() {
        existing.message_id = incoming.message_id.clone();
    }
    if incoming.id.is_some() {
        existing.id = incoming.id.clone();
    }
    if incoming.session_id.is_some() {
        existing.session_id = incoming.session_id.clone();
    }
    if incoming.time.is_some() {
        existing.time = incoming.time.clone();
    }
    if incoming.model.is_some() {
        existing.model = incoming.model.clone();
    }
    if incoming.model_id.is_some() {
        existing.model_id = incoming.model_id.clone();
    }
    if incoming.provider_id.is_some() {
        existing.provider_id = incoming.provider_id.clone();
    }
    if incoming.system.is_some() {
        existing.system = incoming.system;
    }
    if incoming.agent.is_some() {
        existing.agent = incoming.agent.clone();
    }
    if incoming.cost.is_some() {
        existing.cost = incoming.cost;
    }
    if incoming.tokens.is_some() {
        existing.tokens = incoming.tokens.clone();
    }
    if incoming.error.is_some() {
        existing.error = incoming.error.clone();
    }
    if incoming.finish.is_some() {
        existing.finish = incoming.finish.clone();
    }
    if incoming.parent_id.is_some() {
        existing.parent_id = incoming.parent_id.clone();
    }
    if incoming.mode.is_some() {
        existing.mode = incoming.mode.clone();
    }
    if incoming.path.is_some() {
        existing.path = incoming.path.clone();
    }
    if incoming.variant.is_some() {
        existing.variant = incoming.variant.clone();
    }
    // Always update role if non-empty
    if !incoming.role.is_empty() {
        existing.role = incoming.role.clone();
    }
}

/// Upsert a message part. Returns true if the map changed.
pub fn upsert_part(map: &mut MessageMap, part_value: &serde_json::Value) -> bool {
    let part: MessagePart = match serde_json::from_value(part_value.clone()) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let msg_id = part.message_id.as_deref().unwrap_or_default().to_string();
    let part_id = part.id.as_deref().unwrap_or_default().to_string();
    if msg_id.is_empty() {
        return false;
    }

    let msg = map.entry(msg_id).or_insert_with(|| Message {
        info: MessageInfo {
            role: String::new(),
            ..Default::default()
        },
        parts: Vec::new(),
        metadata: None,
    });

    if !part_id.is_empty() {
        if let Some(existing_part) = msg
            .parts
            .iter_mut()
            .find(|p| p.id.as_deref() == Some(&part_id))
        {
            *existing_part = part;
        } else {
            msg.parts.push(part);
        }
    } else {
        msg.parts.push(part);
    }
    true
}

/// Apply a delta (text append) to a message part. Returns true if the map changed.
pub fn apply_part_delta(
    map: &mut MessageMap,
    _session_id: &str,
    message_id: &str,
    part_id: &str,
    field: &str,
    delta: &str,
) -> bool {
    if message_id.is_empty() || part_id.is_empty() || delta.is_empty() {
        return false;
    }
    let msg = match map.get_mut(message_id) {
        Some(m) => m,
        None => return false,
    };
    let part = match msg
        .parts
        .iter_mut()
        .find(|p| p.id.as_deref() == Some(part_id))
    {
        Some(p) => p,
        None => return false,
    };

    match field {
        "text" => {
            let current = part.text.get_or_insert_with(String::new);
            current.push_str(delta);
        }
        _ => {
            // Other fields: treat as text append to the field mapped by name
            // For now, only "text" is supported
            return false;
        }
    }
    true
}

/// Remove a message from the map. Returns true if it existed.
pub fn remove_message(map: &mut MessageMap, message_id: &str) -> bool {
    map.remove(message_id).is_some()
}

/// Remove a part from a message. Returns true if the map changed.
pub fn remove_part(map: &mut MessageMap, message_id: &str, part_id: &str) -> bool {
    if let Some(msg) = map.get_mut(message_id) {
        let before = msg.parts.len();
        msg.parts.retain(|p| p.id.as_deref() != Some(part_id));
        msg.parts.len() != before
    } else {
        false
    }
}
