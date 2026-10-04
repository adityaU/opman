//! Show/hide state shared by every platform: the bar hides only after the
//! cursor has been outside it for [`HIDE_DELAY`].

use std::time::{Duration, Instant};

/// How long the cursor may be outside the bar before it hides again.
pub const HIDE_DELAY: Duration = Duration::from_millis(400);

#[derive(Clone, Copy)]
pub enum Bar {
    Hidden,
    Shown,
    /// Shown, but the cursor left at this instant.
    Leaving(Instant),
}

pub enum Step {
    Stay(Bar),
    Show,
    Hide,
}

pub fn step(bar: Bar, inside: bool, now: Instant) -> Step {
    match (bar, inside) {
        (Bar::Hidden, true) => Step::Show,
        (Bar::Hidden, false) => Step::Stay(Bar::Hidden),
        (Bar::Shown | Bar::Leaving(_), true) => Step::Stay(Bar::Shown),
        (Bar::Shown, false) => Step::Stay(Bar::Leaving(now)),
        (Bar::Leaving(since), false) if now.duration_since(since) >= HIDE_DELAY => Step::Hide,
        (Bar::Leaving(since), false) => Step::Stay(Bar::Leaving(since)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_only_after_the_delay() {
        let start = Instant::now();
        let Step::Stay(leaving) = step(Bar::Shown, false, start) else {
            panic!("leaving must not hide at once");
        };
        assert!(matches!(
            step(leaving, false, start + HIDE_DELAY / 2),
            Step::Stay(Bar::Leaving(_))
        ));
        assert!(matches!(
            step(leaving, false, start + HIDE_DELAY),
            Step::Hide
        ));
    }

    #[test]
    fn returning_cancels_the_hide() {
        let start = Instant::now();
        assert!(matches!(
            step(Bar::Leaving(start), true, start),
            Step::Stay(Bar::Shown)
        ));
        assert!(matches!(step(Bar::Hidden, true, start), Step::Show));
    }
}
