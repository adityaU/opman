use super::*;

#[test]
fn only_the_fallback_asks_for_headless() {
    let on_a_display = Display::Host(":0".into());
    assert!(!on_a_display.chrome_flags().contains(&"--headless=new"));
    assert!(on_a_display
        .chrome_flags()
        .contains(&"--ozone-platform=x11"));
    assert!(Display::Headless.chrome_flags().contains(&"--headless=new"));
}

#[test]
fn a_display_that_can_be_drawn_into_names_itself() {
    assert_eq!(Display::Host(":7".into()).name(), Some(":7"));
    assert_eq!(Display::Headless.name(), None);
}

#[test]
fn a_display_variable_with_nothing_listening_is_not_trusted() {
    // Set but dead is the case that matters: a leftover DISPLAY from a closed session
    // would otherwise send Chromium at an X server that is not there.
    let dead = (FIRST_DISPLAY..=LAST_DISPLAY)
        .find(|n| !listening(*n))
        .expect("some display number in the range is unserved");
    assert!(live_display(&format!(":{dead}")).is_none());
}

#[test]
fn a_screen_number_suffix_still_resolves_to_its_display() {
    // Only the parse is under test; whether :91 is live decides the result.
    assert_eq!(live_display(":91.0").is_some(), listening(91));
}

#[test]
fn a_display_that_is_not_a_number_is_ignored_rather_than_passed_on() {
    assert!(live_display("").is_none());
    assert!(live_display("localhost:0").is_none());
}

#[test]
fn a_lock_from_a_dead_process_is_stale_not_taken() {
    // X writes the pid padded to ten columns. A pid that cannot exist stands in for the
    // crash this has to survive: left as Taken, every crash would retire a display number.
    assert!(matches!(verdict(Some("     999999999\n")), Claim::Stale));
    assert!(matches!(verdict(Some("nonsense")), Claim::Stale));
}

#[test]
fn a_lock_held_by_a_live_process_is_taken() {
    let mine = format!("{:>10}\n", std::process::id());
    assert!(matches!(verdict(Some(&mine)), Claim::Taken));
}

#[test]
fn no_lock_file_at_all_is_a_free_number() {
    assert!(matches!(verdict(None), Claim::Free));
}

#[test]
fn a_leftover_socket_file_is_not_reported_as_listening() {
    let Some(number) = (FIRST_DISPLAY..=LAST_DISPLAY)
        .rev()
        .find(|number| !listening(*number) && !socket(*number).exists())
    else {
        return;
    };
    let path = socket(number);
    let Ok(file) = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    else {
        return; // The socket directory is not writable on this host.
    };
    let cleanup = RemoveFileOnDrop(path);

    assert!(!listening(number));

    drop(file);
    drop(cleanup);
}

#[test]
fn socket_is_bound_reports_pathname_and_abstract_bindings() {
    let number = LAST_DISPLAY;
    let pathname_socket = format!(
        "0000000000000000: 00000002 00000000 00010000 0001 01 12345 \
         /tmp/.X11-unix/X{number}\n"
    );
    let abstract_socket = format!(
        "0000000000000000: 00000002 00000000 00010000 0001 01 67890 \
         @/tmp/.X11-unix/X{number}\n"
    );

    assert!(socket_is_bound(&pathname_socket, number));
    assert!(socket_is_bound(&abstract_socket, number));
}

struct RemoveFileOnDrop(std::path::PathBuf);

impl Drop for RemoveFileOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[tokio::test]
async fn a_virtual_display_lives_and_dies_with_its_handle() {
    let Ok(display) = Display::start_virtual().await else {
        return; // No Xvfb on this host; the fallback path is covered above.
    };
    let name = display
        .name()
        .expect("a virtual display is named")
        .to_owned();
    let number: u8 = name
        .trim_start_matches(':')
        .parse()
        .expect("a display number");
    assert!(listening(number), "it serves while the handle is held");

    drop(display);
    for _ in 0..40 {
        if !listening(number) {
            let _ = std::fs::remove_file(lock(number));
            return;
        }
        tokio::time::sleep(READY_POLL).await;
    }
    panic!("Xvfb on {name} outlived the handle that owned it");
}
