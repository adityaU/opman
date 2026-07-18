use super::*;
use serde_json::json;

#[test]
fn file_browse_query_default_path() {
    let q: FileBrowseQuery = serde_json::from_value(json!({})).unwrap();
    assert_eq!(q.path, "");
    let q2: FileBrowseQuery = serde_json::from_value(json!({"path": "src"})).unwrap();
    assert_eq!(q2.path, "src");
}

#[test]
fn file_entry_and_browse_response_serialize() {
    let e = FileEntry {
        name: "a.rs".into(),
        path: "src/a.rs".into(),
        is_dir: false,
        size: 128,
    };
    let v = serde_json::to_value(&e).unwrap();
    assert_eq!(v["name"], "a.rs");
    assert_eq!(v["is_dir"], false);
    assert_eq!(v["size"], 128);

    let resp = FileBrowseResponse {
        path: "src".into(),
        entries: vec![e],
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["path"], "src");
    assert_eq!(v["entries"].as_array().unwrap().len(), 1);
}

#[test]
fn file_read_query_and_response() {
    let q: FileReadQuery = serde_json::from_value(json!({"path": "a.rs"})).unwrap();
    assert_eq!(q.path, "a.rs");
    let resp = FileReadResponse {
        path: "a.rs".into(),
        content: "fn main(){}".into(),
        language: "rust".into(),
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["language"], "rust");
    assert_eq!(v["content"], "fn main(){}");
}

#[test]
fn file_write_request() {
    let r: FileWriteRequest =
        serde_json::from_value(json!({"path": "a.rs", "content": "x"})).unwrap();
    assert_eq!(r.path, "a.rs");
    assert_eq!(r.content, "x");
}

#[test]
fn editor_lsp_query_optional_coords() {
    let q: EditorLspQuery = serde_json::from_value(json!({
        "path": "a.rs",
        "session_id": "s",
        "line": null,
        "col": null
    }))
    .unwrap();
    assert!(q.line.is_none());
    assert!(q.col.is_none());
    let q2: EditorLspQuery = serde_json::from_value(json!({
        "path": "a.rs",
        "session_id": "s",
        "line": 10,
        "col": 3
    }))
    .unwrap();
    assert_eq!(q2.line, Some(10));
    assert_eq!(q2.col, Some(3));
}

#[test]
fn editor_format_request() {
    let r: EditorFormatRequest =
        serde_json::from_value(json!({"path": "a.rs", "session_id": "s"})).unwrap();
    assert_eq!(r.path, "a.rs");
    assert_eq!(r.session_id, "s");
}

#[test]
fn file_create_request_default_content() {
    let r: FileCreateRequest = serde_json::from_value(json!({"path": "a.rs"})).unwrap();
    assert_eq!(r.content, "");
    let r2: FileCreateRequest =
        serde_json::from_value(json!({"path": "a.rs", "content": "hi"})).unwrap();
    assert_eq!(r2.content, "hi");
}

#[test]
fn dir_and_delete_requests() {
    let d: DirCreateRequest = serde_json::from_value(json!({"path": "d"})).unwrap();
    assert_eq!(d.path, "d");
    let fd: FileDeleteRequest = serde_json::from_value(json!({"path": "f"})).unwrap();
    assert_eq!(fd.path, "f");
    let dd: DirDeleteRequest = serde_json::from_value(json!({"path": "d"})).unwrap();
    assert_eq!(dd.path, "d");
}

#[test]
fn rename_request() {
    let r: RenameRequest =
        serde_json::from_value(json!({"from_path": "a", "to_path": "b"})).unwrap();
    assert_eq!(r.from_path, "a");
    assert_eq!(r.to_path, "b");
}

#[test]
fn file_search_query_default_limit() {
    let q: FileSearchQuery = serde_json::from_value(json!({"q": "foo"})).unwrap();
    assert_eq!(q.q, "foo");
    assert_eq!(q.limit, 20);
    let q2: FileSearchQuery =
        serde_json::from_value(json!({"q": "foo", "limit": 5})).unwrap();
    assert_eq!(q2.limit, 5);
}

#[test]
fn default_search_limit_helper() {
    assert_eq!(default_search_limit(), 20);
}

#[test]
fn file_search_entry_and_response() {
    let e = FileSearchEntry {
        name: "a.rs".into(),
        path: "src/a.rs".into(),
        is_dir: false,
    };
    let resp = FileSearchResponse {
        query: "a".into(),
        entries: vec![e],
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["query"], "a");
    assert_eq!(v["entries"][0]["name"], "a.rs");
}

#[test]
fn download_queries() {
    let f: FileDownloadQuery = serde_json::from_value(json!({"path": "a"})).unwrap();
    assert_eq!(f.path, "a");
    let d: DirDownloadQuery = serde_json::from_value(json!({})).unwrap();
    assert_eq!(d.path, "");
    let d2: DirDownloadQuery = serde_json::from_value(json!({"path": "sub"})).unwrap();
    assert_eq!(d2.path, "sub");
}

#[test]
fn file_upload_response() {
    let r = FileUploadResponse {
        files: vec!["a".into(), "b".into()],
    };
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["files"], json!(["a", "b"]));
}

#[test]
fn doc_read_response_with_spreadsheet() {
    let resp = DocReadResponse {
        path: "x.xlsx".into(),
        data: DocData::Spreadsheet {
            sheets: vec![SheetData {
                name: "Sheet1".into(),
                rows: vec![vec!["a".into(), "b".into()]],
            }],
        },
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["path"], "x.xlsx");
    assert_eq!(v["data"]["type"], "spreadsheet");
    assert_eq!(v["data"]["sheets"][0]["name"], "Sheet1");
}

#[test]
fn doc_data_all_variants_roundtrip() {
    let sheet = DocData::Spreadsheet {
        sheets: vec![SheetData {
            name: "S".into(),
            rows: vec![vec!["1".into()]],
        }],
    };
    let v = serde_json::to_value(&sheet).unwrap();
    assert_eq!(v["type"], "spreadsheet");
    let back: DocData = serde_json::from_value(v).unwrap();
    assert!(matches!(back, DocData::Spreadsheet { .. }));

    let doc = DocData::Document {
        html: "<p>hi</p>".into(),
    };
    let v = serde_json::to_value(&doc).unwrap();
    assert_eq!(v["type"], "document");
    assert_eq!(v["html"], "<p>hi</p>");
    let back: DocData = serde_json::from_value(v).unwrap();
    assert!(matches!(back, DocData::Document { .. }));

    let pres = DocData::Presentation {
        slides: vec![SlideData {
            title: "T".into(),
            content: "C".into(),
        }],
    };
    let v = serde_json::to_value(&pres).unwrap();
    assert_eq!(v["type"], "presentation");
    assert_eq!(v["slides"][0]["title"], "T");
    let back: DocData = serde_json::from_value(v).unwrap();
    assert!(matches!(back, DocData::Presentation { .. }));

    let _ = format!("{doc:?}");
    let _ = doc.clone();
}

#[test]
fn sheet_and_slide_debug_clone() {
    let s = SheetData {
        name: "n".into(),
        rows: vec![],
    };
    let _ = format!("{s:?}");
    let _ = s.clone();
    let sl = SlideData {
        title: "t".into(),
        content: "c".into(),
    };
    let _ = format!("{sl:?}");
    let _ = sl.clone();
}

#[test]
fn doc_write_request() {
    let r: DocWriteRequest = serde_json::from_value(json!({
        "path": "x.docx",
        "data": {"type": "document", "html": "<p>x</p>"}
    }))
    .unwrap();
    assert_eq!(r.path, "x.docx");
    assert!(matches!(r.data, DocData::Document { .. }));
}

#[test]
fn file_edit_entry_and_response() {
    let e = FileEditEntry {
        path: "a.rs".into(),
        original_content: "old".into(),
        new_content: "new".into(),
        timestamp: "t".into(),
        index: 0,
    };
    let v = serde_json::to_value(&e).unwrap();
    assert_eq!(v["original_content"], "old");
    assert_eq!(v["new_content"], "new");
    assert_eq!(v["index"], 0);
    let _ = format!("{e:?}");
    let _ = e.clone();

    let resp = FileEditsResponse {
        session_id: "s".into(),
        edits: vec![e],
        file_count: 1,
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["session_id"], "s");
    assert_eq!(v["file_count"], 1);
    assert_eq!(v["edits"].as_array().unwrap().len(), 1);
}
