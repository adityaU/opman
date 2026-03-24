//! Theme module — applies theme colors to CSS custom properties.
//! Mirrors the TypeScript `utils/theme.ts` applyThemeToCss function.
//! Supports both glassy and flat theme modes, and light/dark/system appearance.

pub mod apply;
pub mod icons;
pub mod system_listener;

pub use apply::*;
pub use system_listener::{install_system_listener, store_theme_pair};
