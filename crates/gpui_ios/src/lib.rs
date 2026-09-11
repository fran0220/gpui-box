//! UIKit native host and portable iOS coordinate contracts.
//!
//! Native acceptance remains pending. `gpui_platform` exposes the explicit
//! font-supplied initializer; generic and headless initialization are refused.
//! The UIKit boundary has no independent document or component state.

pub mod ffi;

#[cfg(any(test, target_os = "ios", feature = "platform-check"))]
mod accessibility;

#[cfg(any(target_os = "ios", feature = "platform-check"))]
mod platform;
#[cfg(any(target_os = "ios", feature = "platform-check"))]
pub use platform::IosPlatform;

/// Creates a UIKit application with an explicit caller-owned font authority.
///
/// Call on the native main thread before entering UIKit's application loop.
/// Initialization is fallible: there is no empty-font or desktop fallback.
/// The selected family must be supplied by `fonts`. Native system-font discovery
/// and headless UIKit application construction are not supported.
#[cfg(any(target_os = "ios", feature = "platform-check"))]
pub fn application(
    fallback_family: &str,
    fonts: Vec<std::borrow::Cow<'static, [u8]>>,
) -> anyhow::Result<gpui::Application> {
    Ok(gpui::Application::with_platform(std::rc::Rc::new(
        IosPlatform::new(fallback_family, fonts)?,
    )))
}
