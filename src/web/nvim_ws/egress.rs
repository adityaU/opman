//! Text-only edit-engine delivery.

use axum::extract::ws::{Message, WebSocket};
use futures::{Sink, SinkExt};
use tokio::sync::mpsc;
use tokio::time::{self, Duration, MissedTickBehavior};

use crate::nvim_ui::stream::wire::ControlMsg;
use crate::nvim_ui::NvimSession;

// Detect a dead embedded Neovim promptly so the editor can leave its usable
// state instead of waiting for the next user action or a long idle interval.
const KEEPALIVE: Duration = Duration::from_secs(1);

pub(crate) async fn run(
    mut sender: futures::stream::SplitSink<WebSocket, Message>,
    mut controls: mpsc::UnboundedReceiver<ControlMsg>,
    session: std::sync::Arc<NvimSession>,
) {
    let mut ticker = time::interval(KEEPALIVE);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            biased;
            control = controls.recv() => {
                let Some(control) = control else { return; };
                if send_control(&mut sender, &control).await.is_err() { return; }
                if matches!(control, ControlMsg::Superseded {} | ControlMsg::TooSlow {} | ControlMsg::Exited { .. }) {
                    let _ = sender.send(Message::Close(None)).await;
                    return;
                }
            }
            _ = ticker.tick() => {
                if !session.is_alive() {
                    let _ = send_control(&mut sender, &ControlMsg::Exited { code: None }).await;
                    let _ = sender.send(Message::Close(None)).await;
                    return;
                }
                if sender.send(Message::Ping(Vec::new().into())).await.is_err() { return; }
            }
        }
    }
}

async fn send_control<S>(sender: &mut S, control: &ControlMsg) -> Result<(), S::Error>
where
    S: Sink<Message> + Unpin,
{
    let json = match serde_json::to_string(control) {
        Ok(json) => json,
        Err(_) => return Ok(()),
    };
    sender.send(Message::Text(json.into())).await
}
