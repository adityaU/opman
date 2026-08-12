use crate::mcp::new_nvim_socket_registry;
use crate::nvim_ui::key::{SessionKey, UiSize};
use crate::nvim_ui::pool::NvimUiPool;
use crate::nvim_ui::rpc::encode::encode_nvim_input;
use crate::nvim_ui::session::{NvimNotification, NvimSession};
use crate::nvim_ui::spawn::ConfigSource;
use rmpv::Value;
use std::collections::VecDeque;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::sync::{broadcast, Mutex, MutexGuard};

static LIVE_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub(super) async fn lock() -> MutexGuard<'static, ()> {
    LIVE_TEST_LOCK.get_or_init(|| Mutex::new(())).lock().await
}

pub(super) fn have_nvim() -> bool {
    std::process::Command::new("nvim")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

pub(super) fn fixture() -> TempDir {
    let project = tempfile::tempdir().expect("temporary Neovim project");
    std::fs::write(
        project.path().join("init.lua"),
        "vim.opt.loadplugins = false\nvim.opt.shadafile = 'NONE'\nvim.opt.swapfile = false\n",
    )
    .expect("minimal Neovim init");
    project
}

pub(super) fn minimal_config(project: &TempDir, id: &str) -> ConfigSource {
    ConfigSource::Minimal {
        init: project.path().join("init.lua"),
        app_name: format!("opman-live-{id}"),
        config_home: project.path().to_path_buf(),
    }
}

pub(super) fn minimal_pool(
    project: &TempDir,
    id: &str,
) -> (NvimUiPool, crate::mcp::NvimSocketRegistry) {
    let registry = new_nvim_socket_registry();
    (
        NvimUiPool::with_config(registry.clone(), minimal_config(project, id)),
        registry,
    )
}

pub(super) async fn start(
    project: &TempDir,
    id: &str,
) -> (Arc<NvimSession>, crate::mcp::NvimSocketRegistry) {
    let registry = new_nvim_socket_registry();
    let session = NvimSession::start_with_config(
        registry.clone(),
        SessionKey::new(0, id),
        project.path(),
        UiSize::default(),
        minimal_config(project, id),
    )
    .await
    .expect("real Neovim session should start");
    (session, registry)
}

const REDRAW_DEADLINE: Duration = Duration::from_secs(30);

pub(super) struct RedrawStream {
    rx: broadcast::Receiver<NvimNotification>,
    pending: VecDeque<(Vec<Value>, Vec<u8>)>,
    seen: VecDeque<String>,
}

impl RedrawStream {
    pub(super) fn new(rx: broadcast::Receiver<NvimNotification>) -> Self {
        Self {
            rx,
            pending: VecDeque::new(),
            seen: VecDeque::new(),
        }
    }

    pub(super) async fn next(&mut self, awaited: &str) -> (Vec<Value>, Vec<u8>) {
        let started = Instant::now();
        self.next_before(awaited, started, started + REDRAW_DEADLINE)
            .await
    }

    pub(super) async fn until<F>(&mut self, awaited: &str, mut matches: F) -> (Vec<Value>, Vec<u8>)
    where
        F: FnMut(&[Value]) -> bool,
    {
        let started = Instant::now();
        loop {
            let batch = self
                .next_before(awaited, started, started + REDRAW_DEADLINE)
                .await;
            if matches(&batch.0) {
                return batch;
            }
        }
    }

    async fn next_before(
        &mut self,
        awaited: &str,
        started: Instant,
        deadline: Instant,
    ) -> (Vec<Value>, Vec<u8>) {
        loop {
            if let Some(batch) = self.pending.pop_front() {
                return batch;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                self.timeout(awaited, started);
            }
            let notification = match tokio::time::timeout(remaining, self.rx.recv()).await {
                Ok(Ok(notification)) => notification,
                Ok(Err(broadcast::error::RecvError::Lagged(count))) => {
                    self.seen.push_back(format!("receiver lagged by {count} notifications"));
                    continue;
                }
                Ok(Err(broadcast::error::RecvError::Closed)) => panic!(
                    "Neovim notification stream closed while waiting for `{awaited}` after {:?}; batches received: {}",
                    started.elapsed(), self.seen_text()
                ),
                Err(_) => self.timeout(awaited, started),
            };
            if notification.method != "redraw" {
                continue;
            }
            let batch = decode_batch(&notification);
            self.seen.push_back(describe_batch(&batch.0));
            self.pending.push_back(batch);
        }
    }

    fn timeout(&self, awaited: &str, started: Instant) -> ! {
        panic!(
            "Neovim redraw timeout waiting for `{awaited}` after {:?}; batches received: {}",
            started.elapsed(),
            self.seen_text()
        )
    }

    fn seen_text(&self) -> String {
        if self.seen.is_empty() {
            return "<none>".into();
        }
        self.seen.iter().cloned().collect::<Vec<_>>().join(" | ")
    }
}

fn decode_batch(notification: &NvimNotification) -> (Vec<Value>, Vec<u8>) {
    let mut input = notification.params.as_slice();
    let value = rmpv::decode::read_value(&mut input).expect("redraw params");
    assert!(input.is_empty());
    (
        value.as_array().cloned().expect("redraw event array"),
        notification.params.clone(),
    )
}

fn describe_batch(events: &[Value]) -> String {
    let names = events
        .iter()
        .filter_map(|event| event.as_array()?.first()?.as_str())
        .collect::<Vec<_>>();
    format!("[{names:?}]")
}

pub(super) fn event<'a>(events: &'a [Value], name: &str) -> Option<&'a [Value]> {
    events.iter().find_map(|value| {
        let fields = value.as_array()?;
        if fields.first()?.as_str()? != name {
            return None;
        }
        fields.get(1)?.as_array().map(Vec::as_slice)
    })
}

pub(super) fn has_event(events: &[Value], name: &str) -> bool {
    event(events, name).is_some()
}

pub(super) fn grid_resize_is(events: &[Value], cols: i64, rows: i64) -> bool {
    event(events, "grid_resize").is_some_and(|args| {
        args.get(1).and_then(Value::as_i64) == Some(cols)
            && args.get(2).and_then(Value::as_i64) == Some(rows)
    })
}

pub(super) fn grid_resize_100x30(events: &[Value]) -> bool {
    grid_resize_is(events, 100, 30)
}
pub(super) fn grid_contains_hello(events: &[Value]) -> bool {
    grid_contains(events, "hello")
}

pub(super) fn grid_contains(events: &[Value], text: &str) -> bool {
    events.iter().any(|value| {
        let Some(fields) = value.as_array() else {
            return false;
        };
        fields.first().and_then(Value::as_str) == Some("grid_line")
            && fields
                .get(1)
                .and_then(Value::as_array)
                .and_then(|args| args.get(3))
                .and_then(Value::as_array)
                .is_some_and(|cells| {
                    let mut row = String::new();
                    for cell in cells {
                        let Some(parts) = cell.as_array() else {
                            continue;
                        };
                        let Some(cell_text) = parts.first().and_then(Value::as_str) else {
                            continue;
                        };
                        let repeat = parts
                            .get(2)
                            .and_then(Value::as_u64)
                            .and_then(|value| usize::try_from(value).ok())
                            .filter(|value| *value > 1)
                            .unwrap_or(1);
                        row.push_str(&cell_text.repeat(repeat));
                    }
                    row.contains(text)
                })
    })
}

pub(super) fn grid_has_row(events: &[Value], row: i64) -> bool {
    events.iter().any(|value| {
        let Some(fields) = value.as_array() else {
            return false;
        };
        fields.first().and_then(Value::as_str) == Some("grid_line")
            && fields
                .get(1)
                .and_then(Value::as_array)
                .and_then(|args| args.get(1))
                .and_then(Value::as_i64)
                == Some(row)
    })
}

pub(super) async fn input(session: &NvimSession, keys: &str) {
    let accepted = session
        .client()
        .request("nvim_input", Value::Array(vec![Value::from(keys)]))
        .await
        .expect("Neovim accepts input");
    assert!(
        accepted.as_i64().is_some_and(|count| count > 0),
        "nvim_input accepted no input: {accepted:?}"
    );
}

pub(super) async fn input_notify(session: &NvimSession, keys: &str) {
    session
        .client()
        .notify(encode_nvim_input(keys).expect("input encodes"))
        .expect("Neovim accepts input");
    tokio::time::sleep(Duration::from_millis(100)).await;
}
