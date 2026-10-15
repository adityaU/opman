//! Which runner opman starts on, given what the user has switched off.

use crate::runner::RunnerKind;

/// Pick the primary runner — the one whose URL becomes the process-wide base URL.
///
/// The CLI's choice wins while it is switched on: `--backend` is an explicit instruction.
/// Switched off, the user's saved default comes next, then the first of `usable` (already
/// filtered to runners that are on and can start). With nothing usable at all the CLI's
/// choice is kept anyway, so opman still starts — every request to it is then refused with
/// a reason instead of opman refusing to start.
pub fn choose_primary(
    cli: RunnerKind,
    cli_enabled: bool,
    saved_default: Option<&RunnerKind>,
    usable: &[RunnerKind],
) -> RunnerKind {
    if cli_enabled {
        return cli;
    }
    if let Some(saved) = saved_default.filter(|saved| usable.contains(saved)) {
        return saved.clone();
    }
    usable.first().cloned().unwrap_or(cli)
}
