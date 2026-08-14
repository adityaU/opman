use super::*;

#[tokio::test]
async fn ws_url_is_read_from_the_devtools_banner() {
    let banner = b"[0813/101112.1:INFO] Something\n\
        DevTools listening on ws://127.0.0.1:41234/devtools/browser/abc-123\n" as &[u8];
    let url = read_ws_url(banner).await.expect("banner parses");
    assert_eq!(url, "ws://127.0.0.1:41234/devtools/browser/abc-123");
}

#[tokio::test]
async fn stderr_without_a_banner_is_an_error_not_a_hang() {
    let noise = b"[0813/101112.1:ERROR] failed to create a profile\n" as &[u8];
    assert!(matches!(
        read_ws_url(noise).await,
        Err(LaunchError::Exited)
    ));
}

#[tokio::test]
async fn a_sandbox_refusal_is_distinguished_so_it_can_be_retried() {
    let fatal = b"[FATAL:zygote_host_impl_linux.cc(128)] No usable sandbox! If you are \
        running on Ubuntu 23.10+ ...\n" as &[u8];
    assert!(matches!(
        read_ws_url(fatal).await,
        Err(LaunchError::SandboxUnavailable)
    ));
}

#[tokio::test]
async fn sandbox_noise_before_a_successful_banner_is_not_a_failure() {
    // Chromium logs plenty on the way up; only an exit without a banner is fatal.
    let mixed = b"No usable sandbox warning from a child\n\
        DevTools listening on ws://127.0.0.1:9/devtools/browser/x\n" as &[u8];
    assert_eq!(
        read_ws_url(mixed).await.expect("the banner still wins"),
        "ws://127.0.0.1:9/devtools/browser/x"
    );
}

#[test]
fn only_the_disabled_variant_passes_sandbox_flags() {
    assert!(Sandbox::Enabled.flags().is_empty());
    assert!(Sandbox::Disabled.flags().contains(&"--no-sandbox"));
}

#[test]
fn which_finds_nothing_for_an_impossible_name() {
    assert!(which("opman-definitely-not-a-real-binary").is_none());
}
