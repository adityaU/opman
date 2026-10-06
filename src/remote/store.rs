//! `~/.config/opman/servers.json`: the remote server list, credentials included.
//!
//! Mode 0600, written tmp-then-rename like the MCP token store, so a crash mid-write never
//! leaves a truncated list and the passwords are never world-readable, not even briefly.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::types::{Credentials, RemoteError, ServerId, ServerRecord, ServerUrl};
use crate::mcp_oauth::Secret;

/// The on-disk shape, kept flat as the contract specifies.
#[derive(Default, Deserialize, Serialize)]
struct Document {
    #[serde(default)]
    servers: Vec<StoredServer>,
}

#[derive(Deserialize, Serialize)]
struct StoredServer {
    id: String,
    name: String,
    url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    password: Option<Secret>,
}

impl StoredServer {
    fn into_record(self) -> Option<ServerRecord> {
        let id = ServerId::parse(&self.id)?;
        let url = ServerUrl::parse(&self.url).ok()?;
        let credentials = match self.username.filter(|u| !u.is_empty()) {
            Some(username) => Credentials::Password {
                username,
                password: self.password.unwrap_or_else(|| Secret::new("")),
            },
            None => Credentials::Open,
        };
        Some(ServerRecord {
            id,
            name: self.name,
            url,
            credentials,
        })
    }

    fn from_record(record: &ServerRecord) -> Self {
        let (username, password) = match &record.credentials {
            Credentials::Open => (None, None),
            Credentials::Password { username, password } => {
                (Some(username.clone()), Some(password.clone()))
            }
        };
        Self {
            id: record.id.as_str().to_owned(),
            name: record.name.clone(),
            url: record.url.as_str().to_owned(),
            username,
            password,
        }
    }
}

enum Backing {
    File(PathBuf),
    /// Nothing persists: no config directory on this platform, or a test.
    Memory,
}

pub struct ServerStore {
    backing: Backing,
}

impl ServerStore {
    /// The user's store, or `None` when the platform has no config directory.
    pub fn open() -> Option<Self> {
        let path = dirs::config_dir()?.join("opman").join("servers.json");
        Some(Self::at(path))
    }

    pub fn at(path: PathBuf) -> Self {
        Self {
            backing: Backing::File(path),
        }
    }

    pub fn memory() -> Self {
        Self {
            backing: Backing::Memory,
        }
    }

    /// Every valid record. A missing file is an empty list; an unreadable entry is
    /// skipped with a warning rather than taking the others down with it.
    pub fn load(&self) -> Vec<ServerRecord> {
        let path = match &self.backing {
            Backing::File(path) => path,
            Backing::Memory => return Vec::new(),
        };
        let Ok(raw) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        if is_too_open(path) {
            tracing::warn!(path = %path.display(), "tightening permissions on servers.json");
            restrict(path);
        }
        let document: Document = match serde_json::from_str(&raw) {
            Ok(document) => document,
            Err(e) => {
                tracing::warn!(path = %path.display(), "ignoring unreadable servers.json: {e}");
                return Vec::new();
            }
        };
        document
            .servers
            .into_iter()
            .filter_map(|stored| {
                let id = stored.id.clone();
                let record = stored.into_record();
                if record.is_none() {
                    tracing::warn!(id, "skipping invalid entry in servers.json");
                }
                record
            })
            .collect()
    }

    pub fn save(&self, records: &[ServerRecord]) -> Result<(), RemoteError> {
        let path = match &self.backing {
            Backing::File(path) => path,
            Backing::Memory => return Ok(()),
        };
        let document = Document {
            servers: records.iter().map(StoredServer::from_record).collect(),
        };
        let body = serde_json::to_vec_pretty(&document)
            .map_err(|e| RemoteError::Store(std::io::Error::other(e)))?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(RemoteError::Store)?;
        }
        let tmp = path.with_extension("json.tmp");
        let mut file = create_private(&tmp).map_err(RemoteError::Store)?;
        file.write_all(&body).map_err(RemoteError::Store)?;
        file.sync_all().map_err(RemoteError::Store)?;
        drop(file);
        std::fs::rename(&tmp, path).map_err(RemoteError::Store)
    }
}

fn create_private(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    // `mode` only applies when the file is created; a stale tmp file keeps its old bits.
    restrict(path);
    Ok(file)
}

fn restrict(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = path;
}

fn is_too_open(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o077 != 0)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod store_tests;
