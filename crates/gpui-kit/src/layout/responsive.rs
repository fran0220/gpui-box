//! Laying out against the space a component was given, not the window.
//!
//! A breakpoint reads the window and is wrong the moment the same component
//! appears in a sidebar, a split pane, a dialog and a full-width page in one
//! application — which is the normal case for a library, not the exotic one.
//! What a component needs is its own width, and GPUI produces that during
//! layout, after the element tree has been built.
//!
//! [`Responsive`] closes that by building its child from the width the
//! previous frame measured, keyed by identity so the measurement survives the
//! rebuild. The cost is one honest frame: the first time a container is laid
//! out nothing has been measured, the child is built from
//! [`ContainerSize::Unmeasured`], and the measurement asks for the frame that
//! uses it. That frame is an animation frame requested during prepaint, so it
//! is the next one rather than the next interaction.
//!
//! What this deliberately does not do is guess. An unmeasured container is not
//! reported as wide, and it is not reported as narrow; it is reported as
//! unmeasured, and the caller decides which arrangement is the better thing to
//! show for one frame. A helper that picked one would be making that decision
//! in the dark and hiding it.

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, Styled, Window,
    div,
};

use crate::foundation::Ident;
use crate::layout::measure;

/// The space a container was given on the previous frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContainerSize {
    /// This container has not been laid out yet. Exactly one frame is spent
    /// here, and the frame that resolves it has already been requested.
    Unmeasured,
    /// Logical pixels, as GPUI laid the container out last frame.
    Measured { width: f32, height: f32 },
}

impl ContainerSize {
    /// The measured width, or `None` on the frame before there is one.
    pub fn width(self) -> Option<f32> {
        match self {
            Self::Unmeasured => None,
            Self::Measured { width, .. } => Some(width),
        }
    }

    /// The measured height, or `None` on the frame before there is one.
    pub fn height(self) -> Option<f32> {
        match self {
            Self::Unmeasured => None,
            Self::Measured { height, .. } => Some(height),
        }
    }

    /// Whether the container is *known* to be at least this wide.
    ///
    /// An unmeasured container answers `false`, which is the conservative
    /// direction: it shows the arrangement that fits in less room for the one
    /// frame before the width is known, rather than the one that might not fit
    /// at all.
    pub fn at_least(self, width: f32) -> bool {
        self.width().is_some_and(|measured| measured >= width)
    }

    /// Whether the container is known to be narrower than this.
    ///
    /// An unmeasured container answers `false` here too, so `at_least` and
    /// `narrower_than` are both false while nothing is known. They are not
    /// each other's negation on purpose: two questions that disagree about an
    /// unmeasured container would let a caller derive a width it does not have.
    pub fn narrower_than(self, width: f32) -> bool {
        self.width().is_some_and(|measured| measured < width)
    }
}

type Build = Box<dyn FnOnce(ContainerSize, &mut Window, &mut App) -> AnyElement>;

/// A container that builds its content from its own measured width.
///
/// ```ignore
/// Responsive::new("settings.body", |size, _, _| {
///     if size.at_least(720.0) {
///         two_columns().into_any_element()
///     } else {
///         one_column().into_any_element()
///     }
/// })
/// ```
#[derive(IntoElement)]
pub struct Responsive {
    ident: Ident,
    build: Build,
}

impl std::fmt::Debug for Responsive {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Responsive")
            .field("ident", &self.ident)
            .finish()
    }
}

impl Responsive {
    pub fn new(
        ident: impl Into<Ident>,
        build: impl FnOnce(ContainerSize, &mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        Self {
            ident: ident.into(),
            build: Box::new(build),
        }
    }
}

impl RenderOnce for Responsive {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let measured = measure::cell(&self.ident.semantic_id(), cx);
        let size = size_of(&measured);
        let content = (self.build)(size, window, cx);

        div()
            .on_children_prepainted({
                let measured = measured.clone();
                move |bounds, window, _| {
                    if let Some(first) = bounds.first() {
                        measure::record(&measured, *first, window);
                    }
                }
            })
            .id(self.ident.element_id())
            .w_full()
            // The measured child is a full-width wrapper rather than the
            // caller's element, so the reading is the room the container had
            // and not the room its content chose to take.
            .child(div().w_full().child(content))
    }
}

/// What the recorded bounds say, treating a zero extent as unmeasured.
///
/// A container that really is zero wide has nothing to lay out either way, so
/// collapsing the two costs nothing and saves carrying a second flag.
pub(crate) fn size_of(
    measured: &std::rc::Rc<std::cell::Cell<gpui::Bounds<gpui::Pixels>>>,
) -> ContainerSize {
    let frame = measured.get();
    let width = f32::from(frame.size.width);
    let height = f32::from(frame.size.height);
    if width <= 0.0 {
        ContainerSize::Unmeasured
    } else {
        ContainerSize::Measured { width, height }
    }
}
