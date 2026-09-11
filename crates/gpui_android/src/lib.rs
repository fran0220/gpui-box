//! Android owns the UI thread and native surface lifetime. See the crate README
//! for the Activity embedding contract and deliberately unsupported capabilities.

mod state;
pub use state::*;

#[cfg(any(test, target_os = "android", feature = "host-check"))]
mod accessibility;

#[cfg(any(target_os = "android", feature = "host-check"))]
mod native;
#[cfg(any(target_os = "android", feature = "host-check"))]
pub use native::{AndroidPlatform, initialize};
