//! Where opman keeps MCP credentials, and how concurrent proxies share them.
//!
//! Tokens live in `~/.config/opman/mcp-auth/<name>.json` at mode 0600, written
//! tmp-then-rename. A file store rather than parent-process ownership because MCP
//! children are spawned by the *runner*: the agent-manager socket is PID-scoped and only
//! reaches a child by env inheritance, the loopback descriptor exists only under `--web`,
//! and a runner keeps its MCP child alive across an opman restart.
//!
//! Refreshes are serialised by an advisory lock on a **sidecar** path — never the token
//! file itself, because the atomic rename swaps that inode out from under any lock held
//! on it. This is correctness, not optimisation: most authorization servers rotate
//! refresh tokens single-use, so two proxies refreshing at once means one presents a
//! consumed token and a strict server revokes the whole grant.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use fs4::fs_std::FileExt;
use serde::{Deserialize, Serialize};

use super::{OAuthError, Secret, ServerName};

/// A token expiring inside this window counts as already expired, so an in-flight
/// request cannot outlive its own credential.
const SKEW_SECS: u64 = 60;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TokenRecord {
    pub version: u8,
    /// Canonical resource URI the token was issued for (RFC 8707).
    pub resource: String,
    /// Issuer, byte-exact from validated metadata, for RFC 9207 comparison.
    pub issuer: String,
    pub client_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<Secret>,
    pub access_token: Secret,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<Secret>,
    pub token_endpoint: String,
    /// Unix seconds. `None` when the server gave no `expires_in`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    #[serde(default)]
    pub granted_scopes: Vec<String>,
    /// Running union of everything ever requested, so a step-up after a restart does not
    /// silently drop scopes granted earlier.
    #[serde(default)]
    pub requested_scopes: Vec<String>,
}

/// The three states a stored credential can be in. Callers match; there is no
/// `is_expired()` anyone can forget to call.
pub enum Credential<'a> {
    Fresh(&'a Secret),
    Refreshable(&'a Secret),
    Unusable,
}

impl TokenRecord {
    pub fn credential(&self, now: u64) -> Credential<'_> {
        let fresh = match self.expires_at {
            None => true,
            Some(at) => at > now.saturating_add(SKEW_SECS),
        };
        if fresh {
            return Credential::Fresh(&self.access_token);
        }
        match &self.refresh_token {
            Some(token) => Credential::Refreshable(token),
            None => Credential::Unusable,
        }
    }
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub struct TokenStore {
    dir: PathBuf,
}

impl TokenStore {
    pub fn open() -> Result<Self, OAuthError> {
        let dir = dirs::config_dir()
            .ok_or(OAuthError::NoConfigDir)?
            .join("opman")
            .join("mcp-auth");
        std::fs::create_dir_all(&dir).map_err(OAuthError::Io)?;
        restrict(&dir, 0o700);
        Ok(Self { dir })
    }

    pub fn with_dir(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn path(&self, name: &ServerName) -> PathBuf {
        self.dir.join(format!("{name}.json"))
    }

    pub fn load(&self, name: &ServerName) -> Option<TokenRecord> {
        let path = self.path(name);
        let raw = std::fs::read_to_string(&path).ok()?;
        // A file left world-readable by an earlier version is repaired rather than
        // trusted silently.
        if is_too_open(&path) {
            tracing::warn!(path = %path.display(), "tightening permissions on token file");
            restrict(&path, 0o600);
        }
        serde_json::from_str(&raw).ok()
    }

    pub fn save(&self, name: &ServerName, record: &TokenRecord) -> Result<(), OAuthError> {
        std::fs::create_dir_all(&self.dir).map_err(OAuthError::Io)?;
        let path = self.path(name);
        let tmp = path.with_extension("json.tmp");
        let mut file = create_private(&tmp)?;
        let body = serde_json::to_vec_pretty(record).map_err(OAuthError::Json)?;
        file.write_all(&body).map_err(OAuthError::Io)?;
        file.sync_all().map_err(OAuthError::Io)?;
        drop(file);
        std::fs::rename(&tmp, &path).map_err(OAuthError::Io)?;
        Ok(())
    }

    pub fn delete(&self, name: &ServerName) -> Result<(), OAuthError> {
        match std::fs::remove_file(self.path(name)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(OAuthError::Io(e)),
        }
    }

    /// Refresh at most once across every process holding this store.
    ///
    /// Double-checked: take the lock, re-read, and only call `refresh` if the record is
    /// *still* stale. Every concurrent proxy but one therefore observes the winner's new
    /// token instead of burning its own single-use refresh token.
    pub async fn refresh_once<F, Fut>(
        &self,
        name: &ServerName,
        now: u64,
        refresh: F,
    ) -> Result<TokenRecord, OAuthError>
    where
        F: FnOnce(TokenRecord) -> Fut,
        Fut: std::future::Future<Output = Result<TokenRecord, OAuthError>>,
    {
        let _lock = RefreshLock::acquire(&self.dir.join(format!("{name}.lock")))?;
        let record = self.load(name).ok_or(OAuthError::LoginRequired)?;
        if matches!(record.credential(now), Credential::Fresh(_)) {
            return Ok(record);
        }
        let refreshed = refresh(record).await?;
        // Written before returning, so a crash cannot lose a rotated refresh token.
        self.save(name, &refreshed)?;
        Ok(refreshed)
    }
}

/// Released on drop, on panic, and on kill — the kernel drops it with the fd.
struct RefreshLock {
    _file: File,
}

impl RefreshLock {
    fn acquire(path: &Path) -> Result<Self, OAuthError> {
        let file = create_private(path)?;
        // Blocking: the whole point is to serialise, and the holder is doing one HTTP
        // round trip. `fs4` wraps the platform call so no `unsafe` is needed here.
        file.lock_exclusive().map_err(OAuthError::Io)?;
        Ok(Self { _file: file })
    }
}

fn create_private(path: &Path) -> Result<File, OAuthError> {
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true).read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).map_err(OAuthError::Io)
}

fn restrict(path: &Path, mode: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
}

fn is_too_open(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        return std::fs::metadata(path)
            .map(|m| m.permissions().mode() & 0o077 != 0)
            .unwrap_or(false);
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

/// How long a proxy waits for another process's refresh before giving up.
pub const LOCK_WAIT: Duration = Duration::from_secs(30);

#[cfg(test)]
#[path = "store_tests.rs"]
mod store_tests;
