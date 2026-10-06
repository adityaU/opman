//! What crosses the proxy and what does not.
//!
//! Hop-by-hop headers describe one connection, not the message, so they are never
//! forwarded (RFC 9110 §7.6.1). Home's own credentials (`authorization`, the
//! `opman_token` cookie, a `?token=` query parameter) stay home: the remote gets home's
//! bearer for *it* instead. A remote's `set-cookie` is dropped so it can never overwrite
//! home's session cookie in the browser.

use axum::http::{header, HeaderMap, HeaderName};

const HOP_BY_HOP: [HeaderName; 8] = [
    header::CONNECTION,
    HeaderName::from_static("keep-alive"),
    header::PROXY_AUTHENTICATE,
    header::PROXY_AUTHORIZATION,
    header::TE,
    header::TRAILER,
    header::TRANSFER_ENCODING,
    header::UPGRADE,
];

/// Headers named in `Connection:` are hop-by-hop for this message too.
fn connection_listed(headers: &HeaderMap) -> Vec<HeaderName> {
    headers
        .get_all(header::CONNECTION)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .filter_map(|name| HeaderName::from_bytes(name.trim().as_bytes()).ok())
        .collect()
}

fn filtered(source: &HeaderMap, also_drop: &[HeaderName]) -> HeaderMap {
    let listed = connection_listed(source);
    let mut out = HeaderMap::with_capacity(source.len());
    for (name, value) in source {
        let dropped = HOP_BY_HOP.contains(name) || also_drop.contains(name) || listed.contains(name);
        if !dropped {
            out.append(name.clone(), value.clone());
        }
    }
    out
}

/// Browser → remote. `host` is the remote's own; `authorization` is replaced.
pub fn upstream_request(source: &HeaderMap) -> HeaderMap {
    filtered(
        source,
        &[header::HOST, header::AUTHORIZATION, header::COOKIE],
    )
}

/// Remote → browser.
pub fn downstream_response(source: &HeaderMap) -> HeaderMap {
    filtered(source, &[header::SET_COOKIE])
}

/// The query string without home's `token=` parameter (EventSource cannot set headers, so
/// the frontend may authenticate SSE through the query). `None` when nothing is left.
pub fn upstream_query(query: Option<&str>) -> Option<String> {
    let kept: Vec<&str> = query?
        .split('&')
        .filter(|pair| !pair.is_empty())
        .filter(|pair| pair.split('=').next() != Some("token"))
        .collect();
    (!kept.is_empty()).then(|| kept.join("&"))
}

#[cfg(test)]
#[path = "headers_tests.rs"]
mod headers_tests;
