//! Theme module — applies theme colors to CSS custom properties.
//! Mirrors the TypeScript `utils/theme.ts` applyThemeToCss function.
//! Supports both glassy and flat theme modes.

pub mod apply;

pub use apply::*;
