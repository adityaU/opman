//! Lines typed by the user, read without losing any.
//!
//! A read from the terminal cannot be cancelled once it starts. If one were started on every
//! `select!` pass and dropped when another branch won, the dropped read would still swallow
//! the next line the user typed. So one read is kept in flight until it finishes.

use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;

pub(super) struct Input {
    source: Source,
}

enum Source {
    Terminal(Option<JoinHandle<Option<String>>>),
    /// Lines supplied by a test.
    Fed(Receiver<String>),
}

impl Input {
    pub(super) fn terminal() -> Self {
        Self {
            source: Source::Terminal(None),
        }
    }

    #[cfg(test)]
    pub(super) fn fed(lines: Receiver<String>) -> Self {
        Self {
            source: Source::Fed(lines),
        }
    }

    /// The next line, or `None` when input has ended. Safe to drop and call again.
    pub(super) async fn line(&mut self) -> Option<String> {
        match &mut self.source {
            Source::Fed(lines) => lines.recv().await,
            Source::Terminal(pending) => {
                let read = pending.get_or_insert_with(|| tokio::task::spawn_blocking(read_line));
                let line = read.await.ok().flatten();
                *pending = None;
                line
            }
        }
    }
}

fn read_line() -> Option<String> {
    let mut line = String::new();
    match std::io::stdin().read_line(&mut line) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line),
    }
}
