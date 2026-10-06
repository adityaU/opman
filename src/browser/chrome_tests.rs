use super::*;

#[test]
fn only_the_disabled_variant_passes_sandbox_flags() {
    assert!(Sandbox::Enabled.flags().is_empty());
    assert!(Sandbox::Disabled.flags().contains(&"--no-sandbox"));
}
