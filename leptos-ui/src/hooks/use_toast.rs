//! Toast hook — re-exports toast context for ergonomic usage.

pub use crate::components::toast::{provide_toast_context, use_toast, ToastContext, ToastType};

/// Convenience type alias used by hooks that take a toast handle.
pub type ToastState = ToastContext;

/// Extension trait for ergonomic toast usage with string-based type.
pub trait ToastExt {
    fn add_typed(&self, message: &str, type_str: &str);
}

impl ToastExt for ToastContext {
    fn add_typed(&self, message: &str, type_str: &str) {
        let tt = match type_str {
            "success" => ToastType::Success,
            "error" => ToastType::Error,
            "warning" => ToastType::Warning,
            _ => ToastType::Info,
        };
        self.add(message, tt, 4000);
    }
}
