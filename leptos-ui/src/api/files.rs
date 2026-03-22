//! File API helpers — browse, read, write, create, delete, upload, raw.

use serde::Serialize;

use super::client::{api_fetch, api_post_void, ApiError};
use crate::types::api::{FileBrowseResponse, FileReadResponse, FileUploadResponse};

/// Browse directory entries.
pub async fn file_browse(path: &str) -> Result<FileBrowseResponse, ApiError> {
    let encoded = js_sys::encode_uri_component(path);
    api_fetch(&format!("/files?path={}", encoded)).await
}

/// Read file content.
pub async fn file_read(path: &str) -> Result<FileReadResponse, ApiError> {
    let encoded = js_sys::encode_uri_component(path);
    api_fetch(&format!("/file/read?path={}", encoded)).await
}

/// Get raw file URL (for binary/image rendering).
pub fn file_raw_url(path: &str) -> String {
    let encoded = js_sys::encode_uri_component(path);
    format!("/api/file/raw?path={}", encoded)
}

/// Get file download URL (triggers browser attachment download).
pub fn file_download_url(path: &str) -> String {
    let encoded = js_sys::encode_uri_component(path);
    format!("/api/file/download?path={}", encoded)
}

/// Get directory download URL (zip archive).
pub fn dir_download_url(path: &str) -> String {
    let encoded = js_sys::encode_uri_component(path);
    format!("/api/dir/download?path={}", encoded)
}

#[derive(Serialize)]
struct WriteBody<'a> {
    path: &'a str,
    content: &'a str,
}

/// Write file content.
pub async fn file_write(path: &str, content: &str) -> Result<(), ApiError> {
    api_post_void("/file/write", &WriteBody { path, content }).await
}

#[derive(Serialize)]
struct CreateBody<'a> {
    path: &'a str,
}

/// Create a new file.
pub async fn file_create(path: &str) -> Result<(), ApiError> {
    api_post_void("/file/create", &CreateBody { path }).await
}

/// Create a new directory.
pub async fn dir_create(path: &str) -> Result<(), ApiError> {
    api_post_void("/dir/create", &CreateBody { path }).await
}

#[derive(Serialize)]
struct DeleteBody<'a> {
    path: &'a str,
}

#[derive(Serialize)]
struct RenameBody<'a> {
    from_path: &'a str,
    to_path: &'a str,
}

/// Rename (move) a file or directory.
pub async fn rename_entry(from_path: &str, to_path: &str) -> Result<(), ApiError> {
    api_post_void("/rename", &RenameBody { from_path, to_path }).await
}

/// Delete a file.
pub async fn file_delete(path: &str) -> Result<(), ApiError> {
    api_post_void("/file/delete", &DeleteBody { path }).await
}

/// Delete a directory.
pub async fn dir_delete(path: &str) -> Result<(), ApiError> {
    api_post_void("/dir/delete", &DeleteBody { path }).await
}

/// Upload files from pre-extracted `File` objects (multipart form).
///
/// The caller must extract files from the `FileList` synchronously before
/// clearing the `<input>` element, because `set_value("")` invalidates the
/// `FileList` reference. This variant accepts a slice of already-extracted
/// `web_sys::File` objects, making it safe to use across an async boundary.
pub async fn file_upload_from_vec(
    dir_path: &str,
    files: &[web_sys::File],
) -> Result<FileUploadResponse, ApiError> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{FormData, Request, RequestCredentials, RequestInit, Response};

    let form = FormData::new()
        .map_err(|e| ApiError { status: 0, message: format!("FormData::new failed: {:?}", e) })?;

    form.append_with_str("directory", dir_path)
        .map_err(|e| ApiError { status: 0, message: format!("append directory failed: {:?}", e) })?;

    for file in files {
        form.append_with_blob_and_filename("files", file, &file.name())
            .map_err(|e| ApiError { status: 0, message: format!("append file failed: {:?}", e) })?;
    }

    let mut opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_credentials(RequestCredentials::SameOrigin);
    opts.set_body(&form);

    let request = Request::new_with_str_and_init("/api/file/upload", &opts)
        .map_err(|e| ApiError { status: 0, message: format!("Request::new failed: {:?}", e) })?;

    let window = web_sys::window()
        .ok_or_else(|| ApiError { status: 0, message: "No window".into() })?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| ApiError { status: 0, message: format!("fetch failed: {:?}", e) })?;

    let resp: Response = resp_value
        .dyn_into()
        .map_err(|_| ApiError { status: 0, message: "Response cast failed".into() })?;

    let status = resp.status();
    if status < 200 || status >= 300 {
        return Err(ApiError { status, message: format!("Upload failed: {}", status) });
    }

    let text = JsFuture::from(
        resp.text().map_err(|_| ApiError { status: 0, message: "text() failed".into() })?,
    )
    .await
    .map_err(|_| ApiError { status: 0, message: "text() promise failed".into() })?
    .as_string()
    .unwrap_or_default();

    serde_json::from_str(&text)
        .map_err(|e| ApiError { status: 0, message: format!("JSON parse error: {}", e) })
}
