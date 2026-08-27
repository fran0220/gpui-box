//! Sound, moving pictures, and geometry, with component and service kept apart.
//!
//! The three surfaces in this module are the ones a general-purpose desktop
//! application needs and cannot assemble out of a button and a slider: a
//! player for audio, a player for video, and a viewer for a 3D model. They
//! keep the posture the rest of the library keeps, which for media means
//! something specific.
//!
//! **A component is not a player service.** GPUI itself has no decoder, audio
//! device, or frame pump. [`AudioPlayer`] and [`VideoPlayer`] therefore stay
//! written against [`MediaTransport`], while [`PlatformMediaTransport`]
//! implements that seam with AVFoundation on macOS and Media Foundation on
//! Windows. A surface with no transport still says exactly that instead of
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
pub mod platform;
pub mod transport;
pub mod video_player;
pub mod waveform;

pub use audio_player::AudioPlayer;
pub use gltf::{ModelBounds, ModelDefect, ModelError, ModelLimit, ModelMesh, ModelScene};
pub use model_viewer::{ModelShading, ModelState, ModelViewer, ModelViewerEvent};
pub use platform::{
    MediaSource, NativeMediaCapabilities, NativeMediaError, NativeMediaEvent, NativeMediaPlayer,
    NativeMediaSubscription, PlatformMediaTransport,
};
pub use transport::{
    FixtureTransport, MediaAvailability, MediaCapabilities, MediaCommand, MediaError,
    MediaErrorKind, MediaEvent, MediaOrigin, MediaOutcome, MediaSnapshot, MediaTransport,
};
pub use video_player::VideoPlayer;
pub use waveform::{AudioWaveform, AudioWaveformState};

use gpui::{AnyElement, Hsla, IntoElement, ParentElement, SharedString, Styled, div, px};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_theme::{ControlSize, Space, Theme, TypeScale};

use crate::foundation::{StyledExt, text};

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
fn notice(
    theme: &Theme,
    tint: Hsla,
    mark: Option<Icon>,
    title: SharedString,
    detail: SharedString,
) -> AnyElement {
    notice_at(theme, tint, mark, title, detail, NoticePlace::Middle)
}

/// The same sentence, placed against whatever is already on the surface.
///
/// The mark says which kind of surface is empty. Without it an empty frame is
/// a rectangle with a sentence in it, and every empty surface in the library
/// looks like the same fault.
fn notice_at(
    theme: &Theme,
    tint: Hsla,
    mark: Option<Icon>,
    title: SharedString,
    detail: SharedString,
    place: NoticePlace,
) -> AnyElement {
    match place {
        NoticePlace::Middle => div()
            .absolute()
            .inset_0()
            .column()
            .items_center()
            .justify_center()
            .gap_token(theme, Space::Xs)
            .p_token(theme, Space::Lg)
            .text_align(gpui::TextAlign::Center)
            .children(mark.map(|mark| {
                icon(mark)
                    .size(px(theme.control.get(ControlSize::Lg).icon_size))
                    .text_color(theme.colors.text_faint)
            }))
            .child(text(theme, TypeScale::Subtitle, title))
            .child(
                text(theme, TypeScale::Body, detail)
                    .max_w(px(theme.measures.readable_width))
                    .text_color(tint),
            )
            .into_any_element(),
        // A still is worth keeping visible, so the sentence takes a band at
        // the foot on a scrim of its own. One line of it: a block deep enough
        // to hold a centred heading and a wrapped paragraph stops being a band
        // over the picture and becomes a second box beside it.
        NoticePlace::Foot => div()
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .row()
            // The heading stands on the first line of the sentence beside it.
            // Baseline alignment put it on the *last* line, so a sentence that
            // wrapped left the heading floating a line below the words it
            // introduces; both children lead on the same line box instead,
            // which holds whether the sentence takes one line or three.
            .items_start()
            .gap_token(theme, Space::Sm)
            .px_token(theme, Space::Md)
            .py_token(theme, Space::Sm)
            .bg(theme.content_veil())
            .child(
                text(theme, TypeScale::Label, title)
                    .flex_none()
                    .line_height(px(theme.typography.caption.line_height)),
            )
            .child(
                text(theme, TypeScale::Caption, detail)
                    .min_w_0()
                    .overflow_hidden()
                    .text_color(tint),
            )
            .into_any_element(),
    }
}
