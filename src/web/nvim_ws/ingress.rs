//! Browser-to-edit-engine JSON ingress. Binary frames are never accepted.

use axum::extract::ws::{Message, WebSocket};
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::nvim_edit::EditEngine;
use crate::nvim_ui::stream::wire::{ClientMsg, ControlMsg};

pub(crate) async fn run(
    mut receiver: futures::stream::SplitStream<WebSocket>,
    engine: Arc<EditEngine>,
    controls: mpsc::UnboundedSender<ControlMsg>,
) {
    while let Some(message) = receiver.next().await {
        let message = match message {
            Ok(message) => message,
            Err(_) => return,
        };
        match message {
            Message::Text(text) => match serde_json::from_str::<ClientMsg>(&text) {
                Ok(message) => {
                    if let Err(error) = engine.handle(message).await {
                        send_error(&controls, &error.to_string());
                    }
                }
                Err(_) => send_error(&controls, "invalid Neovim edit-engine message"),
            },
            Message::Close(_) => return,
            Message::Ping(_) | Message::Pong(_) => {}
            Message::Binary(_) => send_error(&controls, "Neovim edit-engine messages must be text"),
        }
    }
}

fn send_error(controls: &mpsc::UnboundedSender<ControlMsg>, message: &str) {
    let _ = controls.send(ControlMsg::Error {
        message: message.to_owned(),
    });
}

#[cfg(test)]
#[path = "ingress_tests.rs"]
mod ingress_tests;
