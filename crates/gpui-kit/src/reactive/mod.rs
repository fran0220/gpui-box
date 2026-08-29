//! Caller-owned state, and the pipe that connects it to a control.
//!
//! Nothing here renders, and nothing here belongs to a component. A
//! [`Signal`] is a value the caller creates and keeps; a [`Binding`] is a read
//! and a write of that value handed to a control so it can draw what is
//! current and report what was asked for; a [`Form`] is a set of named text
//! signals and the rules the caller judges them by, recorded on the same
//! [`ValidationState`](crate::state::ValidationState) ladder every field
//! control already publishes.
//!
//! The point of the arrangement is that binding changes nothing about what a
//! component is. A bound control still reads caller-owned data and still
//! reports a change; the binding is only the wiring the caller would
//! otherwise write by hand, which is why `.bind` is always additive sugar
//! over a builder's existing value and handler.
//!
//! ```no_run
//! # use gpui::{App, Window};
//! # use gpui_kit::prelude::*;
//! # fn example(cx: &mut App) -> impl gpui::IntoElement {
//! let enabled = Signal::new(cx, true);
//! Switch::new("settings.notify")
//!     .label("Send run notifications")
//!     .bind(&enabled.binding(), cx)
//! # }
//! ```
//!
//! See `docs/reactive.md`.

mod form;
mod history;
mod signal;

pub use form::{Form, FormValues, Rule, validators};
pub use history::History;
pub use signal::{Binding, Signal};
