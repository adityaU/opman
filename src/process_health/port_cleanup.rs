//! Port and socket tracking / cleanup.
//!
//! Reads /proc/net/tcp and /proc/net/tcp6 to find ports owned by the
//! current process (or its children). Reports stale CLOSE_WAIT sockets
//! and tracks listening ports.

use tracing::debug;

use super::{AuditEntry, Mitigation, PortRecord};

/// TCP states in /proc/net/tcp (hex encoded).
const TCP_ESTABLISHED: &str = "01";
const TCP_LISTEN: &str = "0A";
const TCP_CLOSE_WAIT: &str = "08";

/// Scan ports owned by the current process tree.
///
/// Returns `(port_records, audit_entries)`.
pub async fn scan_ports() -> (Vec<PortRecord>, Vec<AuditEntry>) {
    let my_pid = std::process::id();
    let child_pids = collect_child_pids(my_pid);
    let mut records = Vec::new();
    let mut entries = Vec::new();
    let mut close_wait_count = 0u32;

    for path in &["/proc/net/tcp", "/proc/net/tcp6"] {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for line in content.lines().skip(1) {
            if let Some(rec) = parse_tcp_line(line, my_pid, &child_pids) {
                if rec.state == "CLOSE_WAIT" {
                    close_wait_count += 1;
                }
                records.push(rec);
            }
        }
    }

    if close_wait_count > 0 {
        let msg = format!("{} CLOSE_WAIT sockets in process tree", close_wait_count);
        debug!("{}", msg);
        entries.push(AuditEntry::now(
            Mitigation::PortCleanup,
            "scan",
            &msg,
            true,
        ));
    }

    (records, entries)
}

/// Parse a single line from /proc/net/tcp.
/// Returns `Some(PortRecord)` if the socket belongs to our process tree.
fn parse_tcp_line(line: &str, my_pid: u32, child_pids: &[u32]) -> Option<PortRecord> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 10 {
        return None;
    }

    // Column 7 (0-indexed) is the inode, but we need the UID column
    // to filter. Instead, use column 9 which is the inode — we'd need to
    // correlate with /proc/PID/fd. For efficiency, use the simpler approach:
    // read local_address (col 1) and state (col 3).

    let local_addr = parts[1];
    let state_hex = parts[3];

    let port = parse_hex_port(local_addr)?;
    let inode: u64 = parts[9].parse().ok()?;

    // Check if inode belongs to our process tree
    if inode == 0 {
        return None;
    }
    let owner_pid = find_inode_owner(inode, my_pid, child_pids)?;

    let state = match state_hex {
        TCP_ESTABLISHED => "ESTABLISHED",
        TCP_LISTEN => "LISTEN",
        TCP_CLOSE_WAIT => "CLOSE_WAIT",
        "02" => "SYN_SENT",
        "03" => "SYN_RECV",
        "04" => "FIN_WAIT1",
        "05" => "FIN_WAIT2",
        "06" => "TIME_WAIT",
        "07" => "CLOSE",
        "09" => "LAST_ACK",
        "0B" => "CLOSING",
        _ => "UNKNOWN",
    };

    Some(PortRecord {
        port,
        pid: owner_pid,
        state: state.to_string(),
    })
}

/// Parse hex port from "ADDR:PORT" format.
fn parse_hex_port(addr_port: &str) -> Option<u16> {
    let colon = addr_port.rfind(':')?;
    let hex = &addr_port[colon + 1..];
    u16::from_str_radix(hex, 16).ok()
}

/// Collect direct child PIDs of the given parent.
fn collect_child_pids(parent: u32) -> Vec<u32> {
    let mut children = Vec::new();
    let proc_dir = match std::fs::read_dir("/proc") {
        Ok(d) => d,
        Err(_) => return children,
    };

    for entry in proc_dir.flatten() {
        let name = entry.file_name();
        let pid: u32 = match name.to_string_lossy().parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if pid == parent {
            continue;
        }
        let status = format!("/proc/{}/status", pid);
        if let Ok(content) = std::fs::read_to_string(&status) {
            for line in content.lines() {
                if let Some(rest) = line.strip_prefix("PPid:") {
                    let ppid: u32 = rest.trim().parse().unwrap_or(0);
                    if ppid == parent {
                        children.push(pid);
                    }
                    break;
                }
            }
        }
    }
    children
}

/// Find which PID in our tree owns the given socket inode.
fn find_inode_owner(inode: u64, my_pid: u32, child_pids: &[u32]) -> Option<u32> {
    let target = format!("socket:[{}]", inode);
    for &pid in std::iter::once(&my_pid).chain(child_pids.iter()) {
        let fd_path = format!("/proc/{}/fd", pid);
        let dir = match std::fs::read_dir(&fd_path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        for fd_entry in dir.flatten() {
            if let Ok(link) = std::fs::read_link(fd_entry.path()) {
                if link.to_string_lossy() == target {
                    return Some(pid);
                }
            }
        }
    }
    None
}
