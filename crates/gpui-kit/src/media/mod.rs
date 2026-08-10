//! Sound, moving pictures, and geometry — none of it decoded here.
//!
//! The three surfaces in this module are the ones a general-purpose desktop
//! application needs and cannot assemble out of a button and a slider: a
//! player for audio, a player for video, and a viewer for a 3D model. They
//! keep the posture the rest of the library keeps, which for media means
//! something specific.
//!
//! **There is no player in this crate, and there is none in GPUI at the
//! pinned revision.** GPUI draws images and, on macOS, composites a
//! `CVPixelBuffer` through its surface element; it has no decoder, no audio
//! device, and no frame pump. So [`AudioPlayer`] and [`VideoPlayer`] are
//! written against [`MediaTransport`], the seam an operating-system backend
//! lands behind, and a surface with no transport says exactly that instead of
//! drawing a transport bar that would move if anything were playing.
//!
//! **A progress bar is never drawn from a guess.** Position, duration, and
//! buffered spans are the transport's facts. A transport that reports no
//! duration gets a position and no fraction, and a surface with no transport
//! gets no track at all.
//!
//! **A fixture says so.** [`FixtureTransport`] decodes nothing and advances no
//! clock. Every surface publishes and draws [`MediaOrigin`], so a scene, a
//! test, and a screenshot all distinguish a fixture from a player.
//!
//! **A model is read inside a fence.** [`ModelViewer`] takes a glTF 2.0
//! document through the bounded reader in [`gltf`], which accepts a stated
//! subset, refuses anything outside it, and refuses anything past
//! [`ModelBounds`] before allocating for it. The refusal is the contract; the
//! shading is deliberately minimal.

pub mod audio_player;
pub mod gltf;
pub mod model_viewer;
pub mod transport;
pub mod video_player;

pub use audio_player::AudioPlayer;
pub use gltf::{ModelBounds, ModelDefect, ModelError, ModelLimit, ModelMesh, ModelScene};
pub use model_viewer::{ModelShading, ModelState, ModelViewer, ModelViewerEvent};
pub use transport::{
    FixtureTransport, MediaAvailability, MediaCommand, MediaEvent, MediaOrigin, MediaOutcome,
    MediaSnapshot, MediaTransport,
};
pub use video_player::VideoPlayer;

use gpui::{AnyElement, Hsla, IntoElement, ParentElement, SharedString, Styled, div, px};
use gpui_kit_theme::{Space, Theme, TypeScale};

use crate::foundation::StyledExt;

/// Where a notice sits on the surface it covers.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NoticePlace {
    /// The middle of an otherwise empty surface.
    Middle,
    /// A band along the foot, for a surface with a still behind it that the
    /// reader is meant to keep seeing.
    Foot,
}

/// What stands where the media would be when there is none.
///
/// A title and the backend's own sentence, never an empty rectangle: a reader
/// shown a blank frame cannot tell a refusal from silence.
fn notice(theme: &Theme, tint: Hsla, title: SharedString, detail: SharedString) -> AnyElement {
    notice_at(theme, tint, title, detail, NoticePlace::Middle)
}

/// The same sentence, placed against whatever is already on the surface.
fn notice_at(
    theme: &Theme,
    tint: Hsla,
    title: SharedString,
    detail: SharedString,
    place: NoticePlace,
) -> AnyElement {
    let base = match place {
        NoticePlace::Middle => div().absolute().inset_0().justify_center(),
        // A still is worth keeping visible, so the sentence takes a band at
        // the foot on a scrim of its own rather than covering the picture.
        NoticePlace::Foot => div()
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .bg(theme.colors.canvas.opacity(0.88)),
    };
    base.column()
        .items_center()
        .gap_token(theme, Space::Xs)
        .p_token(theme, Space::Lg)
        .text_align(gpui::TextAlign::Center)
        .child(
            div()
                .type_scale(theme, TypeScale::Label)
                .text_color(theme.colors.text)
                .child(title),
        )
        .child(
            div()
                .max_w(px(360.0))
                .type_scale(theme, TypeScale::Caption)
                .text_color(tint)
                .child(detail),
        )
        .into_any_element()
}
