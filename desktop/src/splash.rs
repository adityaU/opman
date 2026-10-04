//! The bundled "Starting opman…" page shown until the backend is up.

use tauri::WebviewWindow;
use url::Url;

pub struct Splash<'a> {
    window: &'a WebviewWindow,
    url: Url,
}

impl<'a> Splash<'a> {
    /// Call before navigating away: remembers the splash page's own URL.
    pub fn capture(window: &'a WebviewWindow) -> tauri::Result<Self> {
        Ok(Self {
            window,
            url: window.url()?,
        })
    }

    /// Shows `message` on the splash page. It travels in the URL fragment rather
    /// than through `eval`, so it lands even if the page has not loaded yet, and
    /// even when the window has already moved on to opman.
    pub fn fail(&self, message: &str) {
        eprintln!("opman-desktop: {message}");
        let mut url = self.url.clone();
        url.set_fragment(Some(&percent_encode(message)));
        if let Err(err) = self.window.navigate(url) {
            eprintln!("opman-desktop: cannot show the error page: {err}");
        }
    }
}

/// Encodes everything but unreserved characters, so `decodeURIComponent` restores it exactly.
fn percent_encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::percent_encode;

    #[test]
    fn encodes_reserved_and_multibyte_characters() {
        assert_eq!(percent_encode("a b%#é\n"), "a%20b%25%23%C3%A9%0A");
    }
}
