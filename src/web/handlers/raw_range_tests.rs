use super::*;

#[test]
fn no_header_means_whole() {
    assert_eq!(parse_range(None, 100), RangeAsk::Whole);
    assert_eq!(parse_range(Some("items=0-1"), 100), RangeAsk::Whole);
    assert_eq!(parse_range(Some("bytes=0-1,5-6"), 100), RangeAsk::Whole);
    assert_eq!(parse_range(Some("bytes=x-1"), 100), RangeAsk::Whole);
}

#[test]
fn closed_open_and_suffix_ranges() {
    let s = |start, end| RangeAsk::Slice(ByteRange { start, end });
    assert_eq!(parse_range(Some("bytes=10-19"), 100), s(10, 19));
    assert_eq!(parse_range(Some("bytes=90-"), 100), s(90, 99));
    assert_eq!(parse_range(Some("bytes=-5"), 100), s(95, 99));
    assert_eq!(parse_range(Some("bytes=50-500"), 100), s(50, 99));
}

#[test]
fn big_open_range_is_capped() {
    let len = 100 * 1024 * 1024;
    assert_eq!(
        parse_range(Some("bytes=0-"), len),
        RangeAsk::Slice(ByteRange {
            start: 0,
            end: MAX_CHUNK - 1
        })
    );
}

#[test]
fn out_of_bounds_is_unsatisfiable() {
    assert_eq!(
        parse_range(Some("bytes=100-"), 100),
        RangeAsk::Unsatisfiable
    );
    assert_eq!(parse_range(Some("bytes=5-2"), 100), RangeAsk::Unsatisfiable);
    assert_eq!(parse_range(Some("bytes=0-1"), 0), RangeAsk::Unsatisfiable);
}

#[test]
fn near_maximum_ranges_do_not_overflow() {
    assert_eq!(
        parse_range(Some("bytes=18446744073709551613-"), u64::MAX),
        RangeAsk::Slice(ByteRange {
            start: u64::MAX - 2,
            end: u64::MAX - 1
        })
    );
}

#[tokio::test]
async fn serves_whole_and_partial() -> anyhow::Result<()> {
    let tmp = tempfile::TempDir::new()?;
    let p = tmp.path().join("a.bin");
    std::fs::write(&p, b"0123456789")?;

    let whole = serve_file(&p, "video/mp4".into(), &HeaderMap::new()).await?;
    assert_eq!(whole.status(), StatusCode::OK);
    assert_eq!(whole.headers()[header::ACCEPT_RANGES], "bytes");
    assert_eq!(whole.headers()[header::CONTENT_TYPE], "video/mp4");
    assert_eq!(whole.headers()[header::CONTENT_LENGTH], "10");
    assert_eq!(
        axum::body::to_bytes(whole.into_body(), 100).await?.as_ref(),
        b"0123456789"
    );

    let mut h = HeaderMap::new();
    h.insert(header::RANGE, HeaderValue::from_static("bytes=2-4"));
    let part = serve_file(&p, "video/mp4".into(), &h).await?;
    assert_eq!(part.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(part.headers()[header::CONTENT_RANGE], "bytes 2-4/10");
    assert_eq!(part.headers()[header::CONTENT_LENGTH], "3");
    let body = axum::body::to_bytes(part.into_body(), 100).await?;
    assert_eq!(&body[..], b"234");

    h.insert(header::RANGE, HeaderValue::from_static("bytes=50-"));
    let bad = serve_file(&p, "video/mp4".into(), &h).await?;
    assert_eq!(bad.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(bad.headers()[header::CONTENT_RANGE], "bytes */10");
    Ok(())
}

#[tokio::test]
async fn directory_is_an_error() -> anyhow::Result<()> {
    let tmp = tempfile::TempDir::new()?;
    let r = serve_file(tmp.path(), "x".into(), &HeaderMap::new()).await;
    assert!(r.is_err());
    Ok(())
}
