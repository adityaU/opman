//! The loopback port opman serves the app on, kept the same from one launch to the next.
//!
//! The webview keeps its storage per origin, and the origin includes the port. The app
//! used to ask the OS for a fresh port every launch, so every launch was a new origin
//! with empty storage: the windows, panes and every preference the page saves were gone
//! after a quit, though a reload kept them. Reusing the last port keeps the origin, so
//! reopening the app is the same as reloading it. A port something else has taken in
//! the meantime is given up for a fresh one — that launch starts clean, as before.

use std::io;
use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Port(u16);

impl Port {
    /// The port the last launch used when it is still free, else a fresh one; either
    /// way it is written back to `file` for the next launch.
    pub fn remembered(file: &Path) -> io::Result<Self> {
        let port = match read(file).filter(|port| is_free(*port)) {
            Some(port) => Self(port),
            None => Self::ephemeral()?,
        };
        // Not remembering only costs the next launch its layout; never fail a start on it.
        if let Err(err) = write(file, port.0) {
            eprintln!("opman-desktop: cannot remember port in {}: {err}", file.display());
        }
        Ok(port)
    }

    /// Asks the OS for a free port. Another process could take it before opman binds
    /// it, but opman then exits and the splash reports it.
    pub fn ephemeral() -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        Ok(Self(listener.local_addr()?.port()))
    }

    pub fn number(self) -> u16 {
        self.0
    }

    pub fn addr(self) -> SocketAddr {
        SocketAddr::from((Ipv4Addr::LOCALHOST, self.0))
    }

    pub fn url(self) -> Result<url::Url, url::ParseError> {
        url::Url::parse(&format!("http://{}/", self.addr()))
    }
}

fn read(file: &Path) -> Option<u16> {
    std::fs::read_to_string(file)
        .ok()?
        .trim()
        .parse()
        .ok()
        .filter(|port| *port != 0)
}

fn write(file: &Path, port: u16) -> io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(file, port.to_string())
}

fn is_free(port: u16) -> bool {
    TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_ok()
}

#[cfg(test)]
#[path = "port_tests.rs"]
mod port_tests;
