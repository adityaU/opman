/// Outcome of the eager session hydration pass.
///
/// A missing default runner is a settled state: there is no upstream session
/// list to read until the first runner-backed action starts one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StartupHydration {
    Pending,
    NoRunner,
    Complete,
}

impl StartupHydration {
    pub(crate) fn is_ready(self) -> bool {
        matches!(self, Self::NoRunner | Self::Complete)
    }
}
