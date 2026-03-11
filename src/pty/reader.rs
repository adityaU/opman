use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::CommandState;

/// Background reader loop: reads bytes from the PTY and feeds them to the parser.
///
/// Also scans for OSC 133 shell integration sequences to track command state:
/// - `\x1b]133;B` → command started (Running)
/// - `\x1b]133;D;0` → command succeeded (Success)
/// - `\x1b]133;D;N` where N≠0 → command failed (Failure)
/// - `\x1b]133;A` → prompt shown (Idle, resets after Success/Failure display)
pub(super) fn read_pty_output(
    mut reader: Box<dyn Read + Send>,
    parser: Arc<Mutex<vt100::Parser>>,
    command_state: Arc<Mutex<CommandState>>,
    dirty: Arc<AtomicBool>,
    last_output_at: Arc<AtomicU64>,
) {
    let mut buf = [0u8; 4096];
    let mut leftover: Vec<u8> = Vec::new();
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let data = if leftover.is_empty() {
                    &buf[..n]
                } else {
                    leftover.extend_from_slice(&buf[..n]);
                    leftover.as_slice()
                };

                scan_osc133(data, &command_state);

                if let Ok(mut p) = parser.lock() {
                    p.process(&buf[..n]);
                }
                dirty.store(true, Ordering::Release);
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                last_output_at.store(now, Ordering::Release);

                // Keep trailing bytes for OSC sequences split across reads
                let keep = data.len().min(32);
                let tail = data[data.len() - keep..].to_vec();
                leftover.clear();
                leftover = tail;
            }
            Err(_) => break,
        }
    }
}

/// Scan a byte slice for OSC 133 shell integration sequences.
///
/// Looks for patterns like `ESC ] 133 ; <cmd> BEL` or `ESC ] 133 ; <cmd> ST`
/// where BEL = 0x07 and ST = ESC \.
pub(super) fn scan_osc133(data: &[u8], command_state: &Arc<Mutex<CommandState>>) {
    // OSC 133 prefix: ESC ] 1 3 3 ;
    const PREFIX: &[u8] = b"\x1b]133;";

    let mut pos = 0;
    while pos + PREFIX.len() < data.len() {
        if let Some(offset) = data[pos..].windows(PREFIX.len()).position(|w| w == PREFIX) {
            tracing::debug!("OSC 133 sequence detected in PTY output");
            let seq_start = pos + offset + PREFIX.len();
            if seq_start >= data.len() {
                break;
            }

            match data[seq_start] {
                // 'A' = prompt start → only reset to Idle if currently Running
                // Keep Success/Failure sticky so dots remain visible until next command
                b'A' => {
                    if let Ok(mut state) = command_state.lock() {
                        tracing::debug!("OSC 133;A (prompt start), current state: {:?}", *state);
                        if *state == CommandState::Running {
                            *state = CommandState::Idle;
                        }
                    }
                }
                b'B' => {
                    tracing::debug!("OSC 133;B (command start) → Running");
                    if let Ok(mut state) = command_state.lock() {
                        *state = CommandState::Running;
                    }
                }
                // 'C' = command output start (ignore, already Running)
                b'C' => {}
                b'D' => {
                    if seq_start + 1 < data.len() && data[seq_start + 1] == b';' {
                        let code_start = seq_start + 2;
                        let mut code_end = code_start;
                        while code_end < data.len() && data[code_end].is_ascii_digit() {
                            code_end += 1;
                        }
                        if code_end > code_start {
                            let code_str =
                                std::str::from_utf8(&data[code_start..code_end]).unwrap_or("1");
                            let exit_code: i32 = code_str.parse().unwrap_or(1);
                            tracing::debug!(
                                "OSC 133;D exit_code={} → {}",
                                exit_code,
                                if exit_code == 0 { "Success" } else { "Failure" }
                            );
                            if let Ok(mut state) = command_state.lock() {
                                *state = if exit_code == 0 {
                                    CommandState::Success
                                } else {
                                    CommandState::Failure
                                };
                            }
                        } else {
                            tracing::debug!("OSC 133;D (no exit code) → Success");
                            if let Ok(mut state) = command_state.lock() {
                                *state = CommandState::Success;
                            }
                        }
                    } else {
                        tracing::debug!("OSC 133;D (no semicolon) → Success");
                        if let Ok(mut state) = command_state.lock() {
                            *state = CommandState::Success;
                        }
                    }
                }
                _ => {}
            }

            pos = seq_start + 1;
        } else {
            break;
        }
    }
}
