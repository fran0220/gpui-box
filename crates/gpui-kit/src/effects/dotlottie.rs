//! Bounded dotLottie playback contracts and semantic cinematic presentation.
//!
//! Hosts resolve animation bytes and cache a prepared clip. Components never
//! fetch a URL, open a path, select an animation filename, or execute an
//! arbitrary program. [`CinematicEffect`] maps an existing [`EffectPlan`] to a
//! policy-owned asset slot, timeline, poster frame, RTL behavior, and particle
//! fallback. The optional `dotlottie` feature supplies a pure-Rust raster
//! adapter; every public contract and fallback remains available without it.

use std::{fmt, rc::Rc, sync::Arc, time::Duration};

use gpui::{
    App, IntoElement, ObjectFit, ParentElement, RenderImage, RenderOnce, SharedString, Styled,
    StyledImage, Window, div, img,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};

use super::{EffectParticles, EffectPlan, EffectPresentation, EffectRecipe};
use crate::foundation::Ident;
use crate::foundation::direction::ActiveDirection;
use crate::motion::keyed;
use crate::strings::{ActiveStrings, StringKey};

/// Absolute ceilings accepted by any Box dotLottie adapter.
///
/// A host may tighten these values through [`DotLottieLimits`], but a backend
/// must reject a configuration above them rather than silently relaxing the
/// security boundary.
pub mod dotlottie_hard_limits {
    use std::time::Duration;

    pub const ENCODED_BYTES: usize = 4 * 1024 * 1024;
    pub const ARCHIVE_ENTRIES: usize = 512;
    pub const ENTRY_BYTES: u64 = 8 * 1024 * 1024;
    pub const EXPANDED_BYTES: u64 = 16 * 1024 * 1024;
    pub const COMPRESSION_RATIO: u32 = 128;
    pub const DIMENSION: u32 = 2_048;
    pub const PIXELS: u64 = 2_097_152;
    pub const FRAME_RATE: u32 = 120;
    pub const FRAMES: u32 = 1_800;
    pub const DURATION: Duration = Duration::from_secs(30);
    pub const ANIMATIONS: u16 = 16;
    pub const STATE_MACHINES: u16 = 16;
    pub const IMAGES: u16 = 64;
    pub const IMAGE_PIXELS: u64 = 4_194_304;
    pub const INPUT_MAGNITUDE: f32 = 1_000_000.0;
}

/// Limits applied before a host-provided `.lottie` archive is prepared.
///
/// Fields are public so a host can make the policy stricter. Values above
/// [`dotlottie_hard_limits`] are invalid and are rejected explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DotLottieLimits {
    pub max_encoded_bytes: usize,
    pub max_archive_entries: usize,
    pub max_entry_bytes: u64,
    pub max_expanded_bytes: u64,
    pub max_compression_ratio: u32,
    pub max_dimension: u32,
    pub max_pixels: u64,
    pub max_frame_rate: u32,
    pub max_frames: u32,
    pub max_duration: Duration,
    pub max_animations: u16,
    pub max_state_machines: u16,
    pub max_images: u16,
    /// Aggregate decoded target pixels across every image asset.
    pub max_image_pixels: u64,
}

impl DotLottieLimits {
    /// The standard runtime boundary: 2 MiB encoded, 8 MiB expanded, a
    /// 1024-pixel edge, 60 fps, 600 frames, eight animations, four state
    /// machines, 32 images, and two million aggregate image pixels.
    pub const fn strict() -> Self {
        Self {
            max_encoded_bytes: 2 * 1024 * 1024,
            max_archive_entries: 256,
            max_entry_bytes: 4 * 1024 * 1024,
            max_expanded_bytes: 8 * 1024 * 1024,
            max_compression_ratio: 64,
            max_dimension: 1_024,
            max_pixels: 1_048_576,
            max_frame_rate: 60,
            max_frames: 600,
            max_duration: Duration::from_secs(10),
            max_animations: 8,
            max_state_machines: 4,
            max_images: 32,
            max_image_pixels: 2_097_152,
        }
    }

    #[cfg(feature = "dotlottie")]
    fn validate(self) -> Result<Self, DotLottieError> {
        use dotlottie_hard_limits as hard;

        let valid = self.max_encoded_bytes > 0
            && self.max_encoded_bytes <= hard::ENCODED_BYTES
            && self.max_archive_entries > 0
            && self.max_archive_entries <= hard::ARCHIVE_ENTRIES
            && self.max_entry_bytes > 0
            && self.max_entry_bytes <= hard::ENTRY_BYTES
            && self.max_expanded_bytes > 0
            && self.max_expanded_bytes <= hard::EXPANDED_BYTES
            && self.max_compression_ratio > 0
            && self.max_compression_ratio <= hard::COMPRESSION_RATIO
            && self.max_dimension > 0
            && self.max_dimension <= hard::DIMENSION
            && self.max_pixels > 0
            && self.max_pixels <= hard::PIXELS
            && self.max_frame_rate > 0
            && self.max_frame_rate <= hard::FRAME_RATE
            && self.max_frames > 0
            && self.max_frames <= hard::FRAMES
            && !self.max_duration.is_zero()
            && self.max_duration <= hard::DURATION
            && self.max_animations > 0
            && self.max_animations <= hard::ANIMATIONS
            && self.max_state_machines <= hard::STATE_MACHINES
            && self.max_images > 0
            && self.max_images <= hard::IMAGES
            && self.max_image_pixels > 0
            && self.max_image_pixels <= hard::IMAGE_PIXELS;
        if valid {
            Ok(self)
        } else {
            Err(DotLottieError::kind(DotLottieErrorKind::InvalidLimits))
        }
    }
}

impl Default for DotLottieLimits {
    fn default() -> Self {
        Self::strict()
    }
}

/// Resolved `.lottie` bytes supplied by the host.
///
/// This type has no URL or path constructor. Asset lookup, authorization,
/// caching, persistence, and transport remain host responsibilities.
#[derive(Clone)]
pub struct DotLottieAsset {
    bytes: Arc<[u8]>,
}

impl DotLottieAsset {
    pub fn new(bytes: impl Into<Arc<[u8]>>) -> Self {
        Self {
            bytes: bytes.into(),
        }
    }

    pub fn encoded_len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl fmt::Debug for DotLottieAsset {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DotLottieAsset")
            .field("encoded_len", &self.encoded_len())
            .finish()
    }
}

/// Stable category for validation, preparation, and rendering failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DotLottieErrorKind {
    RuntimeUnavailable,
    InvalidLimits,
    EmptyAsset,
    EncodedSize,
    ArchiveInvalid,
    ArchiveEntries,
    ArchiveEntrySize,
    ArchiveExpandedSize,
    ArchiveCompressionRatio,
    AnimationCount,
    StateMachineCount,
    ImageCount,
    ImageSize,
    CanvasSize,
    FrameRate,
    FrameCount,
    Duration,
    UnsupportedFeature,
    InvalidInput,
    RenderFailed,
}

impl DotLottieErrorKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::RuntimeUnavailable => "runtime-unavailable",
            Self::InvalidLimits => "invalid-limits",
            Self::EmptyAsset => "empty-asset",
            Self::EncodedSize => "encoded-size",
            Self::ArchiveInvalid => "archive-invalid",
            Self::ArchiveEntries => "archive-entries",
            Self::ArchiveEntrySize => "archive-entry-size",
            Self::ArchiveExpandedSize => "archive-expanded-size",
            Self::ArchiveCompressionRatio => "archive-compression-ratio",
            Self::AnimationCount => "animation-count",
            Self::StateMachineCount => "state-machine-count",
            Self::ImageCount => "image-count",
            Self::ImageSize => "image-size",
            Self::CanvasSize => "canvas-size",
            Self::FrameRate => "frame-rate",
            Self::FrameCount => "frame-count",
            Self::Duration => "duration",
            Self::UnsupportedFeature => "unsupported-feature",
            Self::InvalidInput => "invalid-input",
            Self::RenderFailed => "render-failed",
        }
    }
}

/// Typed adapter failure with an optional host-facing diagnostic.
///
/// Components publish only [`DotLottieErrorKind`] into semantics, so a parser
/// diagnostic cannot leak asset content into snapshots. The detail is for the
/// host's own logs or explicit error UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DotLottieError {
    kind: DotLottieErrorKind,
    detail: SharedString,
}

impl DotLottieError {
    pub fn new(kind: DotLottieErrorKind, detail: impl Into<SharedString>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    pub fn kind(kind: DotLottieErrorKind) -> Self {
        Self::new(kind, kind.name())
    }

    pub fn runtime_unavailable() -> Self {
        Self::kind(DotLottieErrorKind::RuntimeUnavailable)
    }

    pub const fn category(&self) -> DotLottieErrorKind {
        self.kind
    }

    pub fn detail(&self) -> &str {
        self.detail.as_ref()
    }
}

impl fmt::Display for DotLottieError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.kind.name(), self.detail)
    }
}

impl std::error::Error for DotLottieError {}

/// Validated metadata for the animation selected by the archive manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DotLottieMetadata {
    pub width: u32,
    pub height: u32,
    /// Frames per second multiplied by 1000, preserving rates such as 29.97.
    pub frame_rate_millihertz: u32,
    pub frame_count: u32,
    pub duration: Duration,
    pub animation_count: u16,
    pub state_machine_count: u16,
}

impl DotLottieMetadata {
    pub fn frame_rate(self) -> f32 {
        self.frame_rate_millihertz as f32 / 1_000.0
    }
}

/// One exact, normalized sample requested from a prepared clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DotLottieSample {
    progress_per_mille: u16,
    mirror_x: bool,
}

impl DotLottieSample {
    /// Creates an exact sample. Progress must be finite and between zero and
    /// one; invalid values are refused rather than clamped.
    pub fn at_progress(progress: f32, mirror_x: bool) -> Result<Self, DotLottieError> {
        if !progress.is_finite() || !(0.0..=1.0).contains(&progress) {
            return Err(DotLottieError::kind(DotLottieErrorKind::InvalidInput));
        }
        Ok(Self {
            progress_per_mille: (progress * 1_000.0).round() as u16,
            mirror_x,
        })
    }

    pub const fn progress_per_mille(self) -> u16 {
        self.progress_per_mille
    }

    pub const fn mirror_x(self) -> bool {
        self.mirror_x
    }

    fn from_elapsed(elapsed: Duration, duration: Duration, mirror_x: bool) -> Self {
        let elapsed = elapsed.min(duration);
        let denominator = duration.as_nanos().max(1);
        let progress = elapsed.as_nanos().saturating_mul(1_000) / denominator;
        Self {
            progress_per_mille: u16::try_from(progress.min(1_000)).unwrap_or(1_000),
            mirror_x,
        }
    }

    const fn poster(progress_per_mille: u16, mirror_x: bool) -> Self {
        Self {
            progress_per_mille,
            mirror_x,
        }
    }
}

/// Caller-owned transport facts for a stateful dotLottie host runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DotLottiePlaybackState {
    Stopped,
    Playing,
    Paused,
    Completed,
    Unavailable,
}

/// A truthful playback snapshot. It does not mutate or advance itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DotLottiePlayback {
    pub state: DotLottiePlaybackState,
    pub position: Duration,
    pub looping: bool,
}

impl DotLottiePlayback {
    pub const fn new(state: DotLottiePlaybackState, position: Duration) -> Self {
        Self {
            state,
            position,
            looping: false,
        }
    }

    pub const fn looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }
}

/// A bounded state-machine input name and value.
///
/// Names contain at most 64 ASCII letters, digits, dots, dashes, or
/// underscores. Numeric inputs must be finite with an absolute magnitude no
/// greater than [`dotlottie_hard_limits::INPUT_MAGNITUDE`].
#[derive(Debug, Clone, PartialEq)]
pub struct DotLottieInput {
    name: SharedString,
    value: DotLottieInputValue,
}

impl DotLottieInput {
    pub fn new(
        name: impl Into<SharedString>,
        value: DotLottieInputValue,
    ) -> Result<Self, DotLottieError> {
        let name = name.into();
        let valid_name = !name.is_empty()
            && name.len() <= 64
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
        let valid_value = match value {
            DotLottieInputValue::Number(value) => {
                value.is_finite() && value.abs() <= dotlottie_hard_limits::INPUT_MAGNITUDE
            }
            DotLottieInputValue::Boolean(_) | DotLottieInputValue::Trigger => true,
        };
        if !valid_name || !valid_value {
            return Err(DotLottieError::kind(DotLottieErrorKind::InvalidInput));
        }
        Ok(Self { name, value })
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    pub const fn value(&self) -> DotLottieInputValue {
        self.value
    }
}

/// Values a host runtime may route into an already-selected state machine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DotLottieInputValue {
    Boolean(bool),
    Number(f32),
    Trigger,
}

/// Typed requests for a host-owned playback or state-machine runtime.
///
/// These are intents only. They make no claim that playback advanced or that
/// a state-machine transition was accepted.
#[derive(Debug, Clone, PartialEq)]
pub enum DotLottieRequest {
    Play,
    Pause,
    Stop,
    Seek(Duration),
    Input(DotLottieInput),
}

/// A validated clip prepared by a host-selected adapter.
pub trait DotLottieClip: fmt::Debug {
    fn metadata(&self) -> DotLottieMetadata;

    /// Rasterizes one exact normalized sample. Implementations must return a
    /// complete RGBA-derived frame and honor the requested horizontal mirror.
    fn render(&self, sample: DotLottieSample) -> Result<Arc<RenderImage>, DotLottieError>;
}

/// Backend-neutral preparation boundary for resolved host bytes.
pub trait DotLottieAdapter: fmt::Debug {
    fn available(&self) -> bool;

    fn prepare(
        &self,
        asset: DotLottieAsset,
        limits: DotLottieLimits,
    ) -> Result<Rc<dyn DotLottieClip>, DotLottieError>;
}

/// Explicit adapter used when no dotLottie runtime is linked or available.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnavailableDotLottieAdapter;

impl DotLottieAdapter for UnavailableDotLottieAdapter {
    fn available(&self) -> bool {
        false
    }

    fn prepare(
        &self,
        _asset: DotLottieAsset,
        _limits: DotLottieLimits,
    ) -> Result<Rc<dyn DotLottieClip>, DotLottieError> {
        Err(DotLottieError::runtime_unavailable())
    }
}

/// Semantic cinematic recipe selected from an [`EffectPlan`].
///
/// `asset_slot` is a product-neutral lookup key, not a filename. The host maps
/// it to resolved bytes; Box owns duration, poster sampling, and RTL policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CinematicRecipe {
    Arrival,
    Delegation,
    Handoff,
    Aggregation,
    Success,
    Reward,
    Attention,
    Refusal,
    Failure,
}

impl CinematicRecipe {
    pub const fn asset_slot(self) -> &'static str {
        match self {
            Self::Arrival => "arrival",
            Self::Delegation => "delegation",
            Self::Handoff => "handoff",
            Self::Aggregation => "aggregation",
            Self::Success => "success",
            Self::Reward => "reward",
            Self::Attention => "attention",
            Self::Refusal => "refusal",
            Self::Failure => "failure",
        }
    }

    pub const fn duration(self) -> Duration {
        Duration::from_millis(match self {
            Self::Arrival => 900,
            Self::Delegation => 970,
            Self::Handoff => 1_150,
            Self::Aggregation => 900,
            Self::Success => 1_100,
            Self::Reward => 1_680,
            Self::Attention => 700,
            Self::Refusal => 650,
            Self::Failure => 750,
        })
    }

    pub const fn poster_progress_per_mille(self) -> u16 {
        match self {
            Self::Arrival => 680,
            Self::Delegation => 620,
            Self::Handoff => 600,
            Self::Aggregation => 720,
            Self::Success => 720,
            Self::Reward => 700,
            Self::Attention => 500,
            Self::Refusal => 900,
            Self::Failure => 620,
        }
    }

    pub const fn mirrors_in_rtl(self) -> bool {
        matches!(self, Self::Delegation | Self::Handoff)
    }
}

impl From<EffectRecipe> for CinematicRecipe {
    fn from(recipe: EffectRecipe) -> Self {
        match recipe {
            EffectRecipe::ArrivalHalo => Self::Arrival,
            EffectRecipe::DelegationTrace => Self::Delegation,
            EffectRecipe::HandoffTrace => Self::Handoff,
            EffectRecipe::AggregationPulse => Self::Aggregation,
            EffectRecipe::SuccessBurst => Self::Success,
            EffectRecipe::RewardCelebration => Self::Reward,
            EffectRecipe::AttentionPulse => Self::Attention,
            EffectRecipe::RefusalMark => Self::Refusal,
            EffectRecipe::FailurePulse => Self::Failure,
        }
    }
}

impl EffectPlan {
    /// Returns the cinematic asset slot and playback policy for this plan.
    pub fn cinematic_recipe(&self) -> CinematicRecipe {
        self.recipe.into()
    }
}

#[derive(Debug)]
enum CinematicSource {
    Clip(Rc<dyn DotLottieClip>),
    Unavailable(DotLottieError),
}

/// A semantic one-shot effect with an optional prepared dotLottie clip.
///
/// The component fills its parent, owns deterministic playback and poster
/// sampling, mirrors directional clips in RTL, rechecks reduced motion, and
/// falls back to [`EffectParticles`] for every unavailable or rejected clip.
/// It is decorative and installs no hitbox or action handler.
#[derive(Debug, IntoElement)]
pub struct CinematicEffect {
    ident: Ident,
    plan: EffectPlan,
    source: CinematicSource,
    sample_at: Option<Duration>,
}

impl CinematicEffect {
    pub fn new(ident: impl Into<Ident>, plan: EffectPlan) -> Self {
        Self {
            ident: ident.into(),
            plan,
            source: CinematicSource::Unavailable(DotLottieError::runtime_unavailable()),
            sample_at: None,
        }
    }

    pub fn clip(mut self, clip: Rc<dyn DotLottieClip>) -> Self {
        self.source = CinematicSource::Clip(clip);
        self
    }

    /// Applies a host's preparation result without forcing it to branch just
    /// to install the standard fallback.
    pub fn resolved(mut self, clip: Result<Rc<dyn DotLottieClip>, DotLottieError>) -> Self {
        self.source = match clip {
            Ok(clip) => CinematicSource::Clip(clip),
            Err(error) => CinematicSource::Unavailable(error),
        };
        self
    }

    pub fn unavailable(mut self, error: DotLottieError) -> Self {
        self.source = CinematicSource::Unavailable(error);
        self
    }

    /// Samples an exact semantic elapsed time and schedules no frames. Elapsed
    /// times after the policy-owned recipe duration sample its endpoint.
    pub fn sample_at(mut self, elapsed: Duration) -> Self {
        self.sample_at = Some(elapsed);
        self
    }
}

#[derive(Debug, Default)]
struct CinematicClock(Option<web_time::Instant>);

impl RenderOnce for CinematicEffect {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if matches!(self.plan.presentation, EffectPresentation::Suppressed(_)) {
            return div().size_full().into_any_element();
        }

        let recipe = self.plan.cinematic_recipe();
        let static_presentation =
            cx.reduce_motion() || matches!(self.plan.presentation, EffectPresentation::Static(_));
        let mirror_x = cx.is_rtl() && recipe.mirrors_in_rtl();
        let (elapsed, sample, live) = if static_presentation {
            (
                recipe
                    .duration()
                    .mul_f32(f32::from(recipe.poster_progress_per_mille()) / 1_000.0),
                DotLottieSample::poster(recipe.poster_progress_per_mille(), mirror_x),
                false,
            )
        } else if let Some(elapsed) = self.sample_at {
            (
                elapsed.min(recipe.duration()),
                DotLottieSample::from_elapsed(elapsed, recipe.duration(), mirror_x),
                false,
            )
        } else {
            let key: SharedString =
                format!("cinematic-effect:{}:{}", self.plan.surface, self.plan.id).into();
            let slot = keyed::slot::<CinematicClock>(&key, cx);
            let now = cx.background_executor().now();
            let elapsed = {
                let mut clock = slot.borrow_mut();
                let started = *clock.0.get_or_insert(now);
                now.saturating_duration_since(started)
                    .min(recipe.duration())
            };
            (
                elapsed,
                DotLottieSample::from_elapsed(elapsed, recipe.duration(), mirror_x),
                elapsed < recipe.duration(),
            )
        };

        if live {
            window.request_animation_frame();
        }

        let rendered = match &self.source {
            CinematicSource::Clip(clip) => clip.render(sample),
            CinematicSource::Unavailable(error) => Err(error.clone()),
        };
        let (body, text, value) = match rendered {
            Ok(frame) => {
                let text = if static_presentation {
                    cx.strings().text(StringKey::EffectStaticPoster)
                } else {
                    cx.strings().text(StringKey::EffectAdapterFrame)
                };
                let value: SharedString = if static_presentation {
                    "poster"
                } else {
                    "adapter-frame"
                }
                .into();
                (
                    img(frame)
                        .object_fit(ObjectFit::Contain)
                        .size_full()
                        .into_any_element(),
                    text,
                    value,
                )
            }
            Err(error) => {
                let value: SharedString = format!("fallback-{}", error.category().name()).into();
                (
                    EffectParticles::new(self.plan.clone())
                        .sample_at(elapsed)
                        .into_any_element(),
                    cx.strings().text(StringKey::EffectParticleFallback),
                    value,
                )
            }
        };

        div()
            .size_full()
            .child(body)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Image)
                    .text(cx.strings().text(StringKey::EffectCinematic))
                    .description(text)
                    .value(value),
            )
            .into_any_element()
    }
}

#[cfg(feature = "dotlottie")]
mod raster {
    use std::{
        cell::RefCell,
        collections::{HashSet, VecDeque},
        io::{Cursor, Read as _, Write as _},
        rc::Rc,
        sync::Arc,
        time::Duration,
    };

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use gpui::{DevicePixels, RenderImage, size};
    use image::ImageReader;
    use rasterlottie::{Animation, PreparedAnimation, RenderConfig, Renderer};
    use serde::Deserialize;
    use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

    use super::{
        DotLottieAdapter, DotLottieAsset, DotLottieClip, DotLottieError, DotLottieErrorKind,
        DotLottieLimits, DotLottieMetadata, DotLottieSample,
    };

    /// Pure-Rust deterministic dotLottie adapter enabled by the `dotlottie`
    /// Cargo feature.
    #[derive(Debug, Default, Clone, Copy)]
    pub struct RasterDotLottieAdapter;

    impl DotLottieAdapter for RasterDotLottieAdapter {
        fn available(&self) -> bool {
            true
        }

        fn prepare(
            &self,
            asset: DotLottieAsset,
            limits: DotLottieLimits,
        ) -> Result<Rc<dyn DotLottieClip>, DotLottieError> {
            let limits = limits.validate()?;
            if asset.is_empty() {
                return Err(DotLottieError::kind(DotLottieErrorKind::EmptyAsset));
            }
            if asset.encoded_len() > limits.max_encoded_bytes {
                return Err(DotLottieError::kind(DotLottieErrorKind::EncodedSize));
            }
            let archive = validate_archive(asset.bytes(), limits)?;
            let animation =
                Animation::from_dotlottie_bytes(&archive.bytes).map_err(map_prepare_error)?;
            let metadata = validate_animation(&animation, archive.counts, limits)?;
            validate_image_assets(&animation, limits)?;
            let in_point = animation.in_point;
            let prepared = Renderer::default()
                .prepare(&animation)
                .map_err(map_prepare_error)?;
            Ok(Rc::new(RasterDotLottieClip {
                prepared,
                metadata,
                in_point,
                cache: RefCell::new(VecDeque::with_capacity(4)),
            }))
        }
    }

    #[derive(Debug)]
    struct RasterDotLottieClip {
        prepared: PreparedAnimation,
        metadata: DotLottieMetadata,
        in_point: f32,
        cache: RefCell<VecDeque<(u32, bool, Arc<RenderImage>)>>,
    }

    impl DotLottieClip for RasterDotLottieClip {
        fn metadata(&self) -> DotLottieMetadata {
            self.metadata
        }

        fn render(&self, sample: DotLottieSample) -> Result<Arc<RenderImage>, DotLottieError> {
            let last = self.metadata.frame_count.saturating_sub(1);
            let frame_index =
                (u64::from(last) * u64::from(sample.progress_per_mille()) / 1_000) as u32;
            if let Some((_, _, image)) = self.cache.borrow().iter().find(|(cached, mirrored, _)| {
                *cached == frame_index && *mirrored == sample.mirror_x()
            }) {
                return Ok(image.clone());
            }

            let mut frame = self
                .prepared
                .render_frame(self.in_point + frame_index as f32, RenderConfig::default())
                .map_err(map_render_error)?;
            let expected = usize::try_from(frame.width)
                .ok()
                .and_then(|width| {
                    usize::try_from(frame.height)
                        .ok()
                        .and_then(|height| width.checked_mul(height))
                })
                .and_then(|pixels| pixels.checked_mul(4));
            if expected != Some(frame.pixels.len()) {
                return Err(DotLottieError::kind(DotLottieErrorKind::RenderFailed));
            }
            if sample.mirror_x() {
                mirror_rgba(&mut frame.pixels, frame.width, frame.height);
            }
            let width = i32::try_from(frame.width)
                .map_err(|_| DotLottieError::kind(DotLottieErrorKind::CanvasSize))?;
            let height = i32::try_from(frame.height)
                .map_err(|_| DotLottieError::kind(DotLottieErrorKind::CanvasSize))?;
            let image = Arc::new(
                RenderImage::from_rgba(
                    size(DevicePixels(width), DevicePixels(height)),
                    frame.pixels,
                )
                .map_err(|_| DotLottieError::kind(DotLottieErrorKind::RenderFailed))?,
            );
            let mut cache = self.cache.borrow_mut();
            if cache.len() == 4 {
                cache.pop_front();
            }
            cache.push_back((frame_index, sample.mirror_x(), image.clone()));
            Ok(image)
        }
    }

    fn mirror_rgba(pixels: &mut [u8], width: u32, height: u32) {
        let width = width as usize;
        let row_bytes = width * 4;
        for row in pixels.chunks_exact_mut(row_bytes).take(height as usize) {
            for x in 0..width / 2 {
                let left = x * 4;
                let right = (width - x - 1) * 4;
                for channel in 0..4 {
                    row.swap(left + channel, right + channel);
                }
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct ArchiveCounts {
        animations: u16,
        state_machines: u16,
    }

    struct ValidatedArchive {
        bytes: Vec<u8>,
        counts: ArchiveCounts,
    }

    #[derive(Deserialize)]
    struct Manifest {
        #[serde(default)]
        animations: Vec<serde_json::Value>,
        #[serde(default, rename = "stateMachines", alias = "state_machines")]
        state_machines: Vec<serde_json::Value>,
    }

    fn validate_archive(
        bytes: &[u8],
        limits: DotLottieLimits,
    ) -> Result<ValidatedArchive, DotLottieError> {
        let mut archive = ZipArchive::new(Cursor::new(bytes))
            .map_err(|_| DotLottieError::kind(DotLottieErrorKind::ArchiveInvalid))?;
        if archive.is_empty() || archive.len() > limits.max_archive_entries {
            return Err(DotLottieError::kind(DotLottieErrorKind::ArchiveEntries));
        }

        let mut expanded = 0u64;
        let mut manifest_bytes = None;
        let mut manifest_count = 0usize;
        let mut names = HashSet::with_capacity(archive.len());
        let mut validated_entries = Vec::with_capacity(archive.len());
        for index in 0..archive.len() {
            let mut file = archive
                .by_index(index)
                .map_err(|_| DotLottieError::kind(DotLottieErrorKind::ArchiveInvalid))?;
            let Some(path) = file.enclosed_name() else {
                return Err(DotLottieError::kind(DotLottieErrorKind::ArchiveInvalid));
            };
            let name = path.to_string_lossy().into_owned();
            let is_symlink = file
                .unix_mode()
                .is_some_and(|mode| mode & 0o170_000 == 0o120_000);
            if file.encrypted() || is_symlink || !names.insert(name.clone()) {
                return Err(DotLottieError::kind(DotLottieErrorKind::ArchiveInvalid));
            }
            let entry_size = file.size();
            if entry_size > limits.max_entry_bytes {
                return Err(DotLottieError::kind(DotLottieErrorKind::ArchiveEntrySize));
            }
            expanded = expanded
                .checked_add(entry_size)
                .ok_or_else(|| DotLottieError::kind(DotLottieErrorKind::ArchiveExpandedSize))?;
            if expanded > limits.max_expanded_bytes {
                return Err(DotLottieError::kind(
                    DotLottieErrorKind::ArchiveExpandedSize,
                ));
            }
            if entry_size > 0 {
                let compressed = file.compressed_size();
                if compressed == 0
                    || entry_size
                        > compressed.saturating_mul(u64::from(limits.max_compression_ratio))
                {
                    return Err(DotLottieError::kind(
                        DotLottieErrorKind::ArchiveCompressionRatio,
                    ));
                }
            }
            if file.is_dir() {
                continue;
            }
            let mut contents = Vec::with_capacity(entry_size.min(limits.max_entry_bytes) as usize);
            file.by_ref()
                .take(limits.max_entry_bytes + 1)
                .read_to_end(&mut contents)
                .map_err(|_| DotLottieError::kind(DotLottieErrorKind::ArchiveInvalid))?;
            if contents.len() as u64 != entry_size {
                return Err(DotLottieError::kind(DotLottieErrorKind::ArchiveInvalid));
            }
            if name == "manifest.json" {
                manifest_count += 1;
                manifest_bytes = Some(contents.clone());
            }
            validated_entries.push((name, contents));
        }
        if manifest_count != 1 {
            return Err(DotLottieError::kind(DotLottieErrorKind::ArchiveInvalid));
        }
        let manifest = serde_json::from_slice::<Manifest>(
            manifest_bytes
                .as_deref()
                .ok_or_else(|| DotLottieError::kind(DotLottieErrorKind::ArchiveInvalid))?,
        )
        .map_err(|_| DotLottieError::kind(DotLottieErrorKind::ArchiveInvalid))?;
        let animations = u16::try_from(manifest.animations.len())
            .map_err(|_| DotLottieError::kind(DotLottieErrorKind::AnimationCount))?;
        if animations == 0 || animations > limits.max_animations {
            return Err(DotLottieError::kind(DotLottieErrorKind::AnimationCount));
        }
        let state_machines = u16::try_from(manifest.state_machines.len())
            .map_err(|_| DotLottieError::kind(DotLottieErrorKind::StateMachineCount))?;
        if state_machines > limits.max_state_machines {
            return Err(DotLottieError::kind(DotLottieErrorKind::StateMachineCount));
        }
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        for (name, contents) in validated_entries {
            writer
                .start_file(name, options)
                .map_err(|_| DotLottieError::kind(DotLottieErrorKind::ArchiveInvalid))?;
            writer
                .write_all(&contents)
                .map_err(|_| DotLottieError::kind(DotLottieErrorKind::ArchiveInvalid))?;
        }
        let bytes = writer
            .finish()
            .map_err(|_| DotLottieError::kind(DotLottieErrorKind::ArchiveInvalid))?
            .into_inner();
        Ok(ValidatedArchive {
            bytes,
            counts: ArchiveCounts {
                animations,
                state_machines,
            },
        })
    }

    fn validate_animation(
        animation: &Animation,
        counts: ArchiveCounts,
        limits: DotLottieLimits,
    ) -> Result<DotLottieMetadata, DotLottieError> {
        if animation.width == 0
            || animation.height == 0
            || animation.width > limits.max_dimension
            || animation.height > limits.max_dimension
            || u64::from(animation.width) * u64::from(animation.height) > limits.max_pixels
        {
            return Err(DotLottieError::kind(DotLottieErrorKind::CanvasSize));
        }
        if !animation.frame_rate.is_finite()
            || animation.frame_rate <= 0.0
            || animation.frame_rate > limits.max_frame_rate as f32
        {
            return Err(DotLottieError::kind(DotLottieErrorKind::FrameRate));
        }
        let frames = animation.duration_frames();
        if !animation.in_point.is_finite()
            || !animation.out_point.is_finite()
            || !frames.is_finite()
            || frames <= 0.0
            || frames.ceil() > limits.max_frames as f32
        {
            return Err(DotLottieError::kind(DotLottieErrorKind::FrameCount));
        }
        let duration_seconds = f64::from(frames) / f64::from(animation.frame_rate);
        if !duration_seconds.is_finite()
            || duration_seconds <= 0.0
            || duration_seconds > limits.max_duration.as_secs_f64()
        {
            return Err(DotLottieError::kind(DotLottieErrorKind::Duration));
        }
        Ok(DotLottieMetadata {
            width: animation.width,
            height: animation.height,
            frame_rate_millihertz: (animation.frame_rate * 1_000.0).round() as u32,
            frame_count: frames.ceil() as u32,
            duration: Duration::from_secs_f64(duration_seconds),
            animation_count: counts.animations,
            state_machine_count: counts.state_machines,
        })
    }

    fn validate_image_assets(
        animation: &Animation,
        limits: DotLottieLimits,
    ) -> Result<(), DotLottieError> {
        let images = animation
            .assets
            .iter()
            .filter(|asset| asset.is_image_asset())
            .collect::<Vec<_>>();
        if images.len() > usize::from(limits.max_images) {
            return Err(DotLottieError::kind(DotLottieErrorKind::ImageCount));
        }

        let mut target_pixels = 0u64;
        for asset in images {
            if let (Some(width), Some(height)) = (asset.width, asset.height) {
                validate_image_dimensions(width, height, limits)?;
            }
            let data_url = asset
                .image_data_url()
                .ok_or_else(|| DotLottieError::kind(DotLottieErrorKind::UnsupportedFeature))?;
            let (metadata, payload) = data_url
                .split_once(',')
                .ok_or_else(|| DotLottieError::kind(DotLottieErrorKind::ArchiveInvalid))?;
            if !metadata.starts_with("data:image/") || !metadata.contains(";base64") {
                return Err(DotLottieError::kind(DotLottieErrorKind::ArchiveInvalid));
            }
            let payload = payload.trim();
            let max_payload = limits.max_entry_bytes.saturating_mul(4) / 3 + 4;
            if payload.len() as u64 > max_payload {
                return Err(DotLottieError::kind(DotLottieErrorKind::ArchiveEntrySize));
            }
            let bytes = STANDARD
                .decode(payload)
                .map_err(|_| DotLottieError::kind(DotLottieErrorKind::ArchiveInvalid))?;
            if bytes.len() as u64 > limits.max_entry_bytes {
                return Err(DotLottieError::kind(DotLottieErrorKind::ArchiveEntrySize));
            }
            let (source_width, source_height) = ImageReader::new(Cursor::new(&bytes))
                .with_guessed_format()
                .map_err(|_| DotLottieError::kind(DotLottieErrorKind::ArchiveInvalid))?
                .into_dimensions()
                .map_err(|_| DotLottieError::kind(DotLottieErrorKind::ArchiveInvalid))?;
            validate_image_dimensions(source_width, source_height, limits)?;
            let target_width = asset.width.unwrap_or(source_width);
            let target_height = asset.height.unwrap_or(source_height);
            let pixels = validate_image_dimensions(target_width, target_height, limits)?;
            target_pixels = target_pixels
                .checked_add(pixels)
                .ok_or_else(|| DotLottieError::kind(DotLottieErrorKind::ImageSize))?;
            if target_pixels > limits.max_image_pixels {
                return Err(DotLottieError::kind(DotLottieErrorKind::ImageSize));
            }
        }
        Ok(())
    }

    fn validate_image_dimensions(
        width: u32,
        height: u32,
        limits: DotLottieLimits,
    ) -> Result<u64, DotLottieError> {
        let pixels = u64::from(width) * u64::from(height);
        if width == 0
            || height == 0
            || width > limits.max_dimension
            || height > limits.max_dimension
            || pixels > limits.max_image_pixels
        {
            Err(DotLottieError::kind(DotLottieErrorKind::ImageSize))
        } else {
            Ok(pixels)
        }
    }

    fn map_prepare_error(error: rasterlottie::RasterlottieError) -> DotLottieError {
        let kind = match error {
            rasterlottie::RasterlottieError::UnsupportedFeatures { .. } => {
                DotLottieErrorKind::UnsupportedFeature
            }
            rasterlottie::RasterlottieError::Parse(_)
            | rasterlottie::RasterlottieError::DotLottieArchive(_)
            | rasterlottie::RasterlottieError::InvalidDotLottie { .. } => {
                DotLottieErrorKind::ArchiveInvalid
            }
            rasterlottie::RasterlottieError::InvalidCanvasSize { .. } => {
                DotLottieErrorKind::CanvasSize
            }
            _ => DotLottieErrorKind::RenderFailed,
        };
        DotLottieError::kind(kind)
    }

    fn map_render_error(_error: rasterlottie::RasterlottieError) -> DotLottieError {
        DotLottieError::kind(DotLottieErrorKind::RenderFailed)
    }

    #[cfg(test)]
    mod tests {
        use std::{io::Write as _, sync::Arc};

        use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

        use super::*;

        fn archive(manifest: &str, animation: &str) -> Vec<u8> {
            let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
            let options =
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            writer
                .start_file("manifest.json", options)
                .expect("fixture manifest entry");
            writer
                .write_all(manifest.as_bytes())
                .expect("fixture manifest bytes");
            writer
                .start_file("animations/main.json", options)
                .expect("fixture animation entry");
            writer
                .write_all(animation.as_bytes())
                .expect("fixture animation bytes");
            writer.finish().expect("fixture archive").into_inner()
        }

        fn archive_with_entries(
            entries: &[(&str, &[u8])],
            compression: CompressionMethod,
        ) -> Vec<u8> {
            let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
            let options = SimpleFileOptions::default().compression_method(compression);
            for (name, contents) in entries {
                writer.start_file(*name, options).expect("fixture entry");
                writer.write_all(contents).expect("fixture entry bytes");
            }
            writer.finish().expect("fixture archive").into_inner()
        }

        fn animation(width: u32, height: u32, frame_rate: f32, frames: f32) -> String {
            format!(
                r#"{{"v":"5.7.6","fr":{frame_rate},"ip":0,"op":{frames},"w":{width},"h":{height},"layers":[{{"nm":"Shape","ind":1,"ty":4,"shapes":[{{"ty":"gr","it":[{{"ty":"rc","p":{{"a":0,"k":[4,8]}},"s":{{"a":0,"k":[4,8]}},"r":{{"a":0,"k":0}}}},{{"ty":"fl","c":{{"a":0,"k":[1,0,0,1]}},"o":{{"a":0,"k":100}}}},{{"ty":"tr","a":{{"a":0,"k":[0,0]}},"p":{{"a":0,"k":[0,0]}},"s":{{"a":0,"k":[100,100]}},"r":{{"a":0,"k":0}},"o":{{"a":0,"k":100}}}}]}}]}}]}}"#
            )
        }

        fn animation_with_png(source_width: u32, source_height: u32) -> String {
            let image = image::RgbaImage::new(source_width, source_height);
            let mut bytes = Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(image)
                .write_to(&mut bytes, image::ImageFormat::Png)
                .expect("fixture image encodes");
            let encoded = STANDARD.encode(bytes.into_inner());
            format!(
                r#"{{"v":"5.7.6","fr":30,"ip":0,"op":60,"w":16,"h":16,"assets":[{{"id":"image","w":1,"h":1,"p":"data:image/png;base64,{encoded}","e":1}}],"layers":[{{"nm":"Image","ind":1,"ty":2,"refId":"image"}}]}}"#
            )
        }

        #[test]
        fn adapter_validates_metadata_and_reuses_deterministic_frames() {
            let bytes = archive(
                r#"{"animations":[{"id":"main"}],"stateMachines":[{"id":"idle"}]}"#,
                &animation(16, 16, 30.0, 60.0),
            );
            let clip = RasterDotLottieAdapter
                .prepare(DotLottieAsset::new(bytes), DotLottieLimits::strict())
                .expect("valid archive prepares");
            assert_eq!(
                clip.metadata(),
                DotLottieMetadata {
                    width: 16,
                    height: 16,
                    frame_rate_millihertz: 30_000,
                    frame_count: 60,
                    duration: Duration::from_secs(2),
                    animation_count: 1,
                    state_machine_count: 1,
                }
            );
            let sample = DotLottieSample::at_progress(0.5, false).expect("valid sample");
            let first = clip.render(sample).expect("first frame");
            let second = clip.render(sample).expect("cached frame");
            assert!(Arc::ptr_eq(&first, &second));

            let clip = RasterDotLottieAdapter
                .prepare(
                    DotLottieAsset::new(archive(
                        r#"{"animations":[{"id":"main"}]}"#,
                        &animation_with_png(2, 2),
                    )),
                    DotLottieLimits::strict(),
                )
                .expect("bounded embedded image prepares");
            clip.render(sample).expect("bounded image frame renders");
        }

        #[test]
        fn adapter_rejects_every_bounded_metadata_family() {
            let cases = [
                (
                    animation(2_048, 16, 30.0, 60.0),
                    DotLottieErrorKind::CanvasSize,
                ),
                (animation(16, 16, 61.0, 60.0), DotLottieErrorKind::FrameRate),
                (
                    animation(16, 16, 30.0, 601.0),
                    DotLottieErrorKind::FrameCount,
                ),
            ];
            for (animation, expected) in cases {
                let result = RasterDotLottieAdapter.prepare(
                    DotLottieAsset::new(archive(r#"{"animations":[{"id":"main"}]}"#, &animation)),
                    DotLottieLimits::strict(),
                );
                assert_eq!(
                    result.expect_err("metadata must be rejected").category(),
                    expected
                );
            }

            let too_many = r#"{"animations":[{},{},{},{},{},{},{},{},{}]}"#;
            let result = RasterDotLottieAdapter.prepare(
                DotLottieAsset::new(archive(too_many, &animation(16, 16, 30.0, 60.0))),
                DotLottieLimits::strict(),
            );
            assert_eq!(
                result
                    .expect_err("animation count must be rejected")
                    .category(),
                DotLottieErrorKind::AnimationCount
            );
        }

        #[test]
        fn horizontal_mirroring_is_part_of_the_sample_contract() {
            let bytes = archive(
                r#"{"animations":[{"id":"main"}]}"#,
                &animation(16, 16, 30.0, 60.0),
            );
            let clip = RasterDotLottieAdapter
                .prepare(DotLottieAsset::new(bytes), DotLottieLimits::strict())
                .expect("valid archive prepares");
            let normal = clip
                .render(DotLottieSample::at_progress(0.0, false).expect("normal sample"))
                .expect("normal frame");
            let mirrored = clip
                .render(DotLottieSample::at_progress(0.0, true).expect("mirrored sample"))
                .expect("mirrored frame");
            let normal = normal.as_bytes(0).expect("normal RGBA bytes");
            let mirrored = mirrored.as_bytes(0).expect("mirrored RGBA bytes");
            for y in 0..16usize {
                for x in 0..16usize {
                    let left = (y * 16 + x) * 4;
                    let right = (y * 16 + (15 - x)) * 4;
                    assert_eq!(&normal[left..left + 4], &mirrored[right..right + 4]);
                }
            }
        }

        #[test]
        fn archive_admission_rejects_empty_oversized_ambiguous_and_bomb_inputs() {
            let strict = DotLottieLimits::strict();
            let cases = [
                (
                    RasterDotLottieAdapter.prepare(DotLottieAsset::new(Vec::<u8>::new()), strict),
                    DotLottieErrorKind::EmptyAsset,
                ),
                (
                    RasterDotLottieAdapter.prepare(DotLottieAsset::new(vec![0; 32]), strict),
                    DotLottieErrorKind::ArchiveInvalid,
                ),
                (
                    RasterDotLottieAdapter.prepare(
                        DotLottieAsset::new(vec![0; strict.max_encoded_bytes + 1]),
                        strict,
                    ),
                    DotLottieErrorKind::EncodedSize,
                ),
            ];
            for (result, expected) in cases {
                assert_eq!(
                    result.expect_err("archive must be rejected").category(),
                    expected
                );
            }

            let manifest = br#"{"animations":[{"id":"main"}]}"#;
            let animation = animation(16, 16, 30.0, 60.0);
            let traversal = archive_with_entries(
                &[
                    ("manifest.json", manifest),
                    ("../animations/main.json", animation.as_bytes()),
                ],
                CompressionMethod::Stored,
            );
            let result = RasterDotLottieAdapter.prepare(DotLottieAsset::new(traversal), strict);
            assert_eq!(
                result.expect_err("traversal must be rejected").category(),
                DotLottieErrorKind::ArchiveInvalid
            );

            let zeros = vec![0; 64 * 1024];
            let bomb = archive_with_entries(
                &[
                    ("manifest.json", manifest),
                    ("animations/main.json", animation.as_bytes()),
                    ("images/repetitive.bin", &zeros),
                ],
                CompressionMethod::Deflated,
            );
            let result = RasterDotLottieAdapter.prepare(DotLottieAsset::new(bomb), strict);
            assert_eq!(
                result
                    .expect_err("compression bomb must be rejected")
                    .category(),
                DotLottieErrorKind::ArchiveCompressionRatio
            );
        }

        #[test]
        fn hard_limit_expansion_and_duration_families_are_refused_explicitly() {
            let mut invalid = DotLottieLimits::strict();
            invalid.max_dimension = super::super::dotlottie_hard_limits::DIMENSION + 1;
            let result = RasterDotLottieAdapter.prepare(
                DotLottieAsset::new(archive(
                    r#"{"animations":[{"id":"main"}]}"#,
                    &animation(16, 16, 30.0, 60.0),
                )),
                invalid,
            );
            assert_eq!(
                result.expect_err("hard limits must be rejected").category(),
                DotLottieErrorKind::InvalidLimits
            );

            let result = RasterDotLottieAdapter.prepare(
                DotLottieAsset::new(archive(
                    r#"{"animations":[{"id":"main"}]}"#,
                    &animation(16, 16, 1.0, 20.0),
                )),
                DotLottieLimits::strict(),
            );
            assert_eq!(
                result.expect_err("duration must be rejected").category(),
                DotLottieErrorKind::Duration
            );

            let manifest = r#"{"animations":[{"id":"main"}],"stateMachines":[{},{},{},{},{}]}"#;
            let result = RasterDotLottieAdapter.prepare(
                DotLottieAsset::new(archive(manifest, &animation(16, 16, 30.0, 60.0))),
                DotLottieLimits::strict(),
            );
            assert_eq!(
                result
                    .expect_err("state-machine count must be rejected")
                    .category(),
                DotLottieErrorKind::StateMachineCount
            );

            let mut one_image_pixel = DotLottieLimits::strict();
            one_image_pixel.max_image_pixels = 1;
            let result = RasterDotLottieAdapter.prepare(
                DotLottieAsset::new(archive(
                    r#"{"animations":[{"id":"main"}]}"#,
                    &animation_with_png(2, 2),
                )),
                one_image_pixel,
            );
            assert_eq!(
                result
                    .expect_err("decoded image dimensions must be rejected")
                    .category(),
                DotLottieErrorKind::ImageSize
            );

            let image_asset =
                r#"{"id":"image","w":1,"h":1,"p":"data:image/png;base64,AAAA","e":1}"#;
            let assets = std::iter::repeat_n(image_asset, 33)
                .collect::<Vec<_>>()
                .join(",");
            let too_many_images = format!(
                r#"{{"v":"5.7.6","fr":30,"ip":0,"op":60,"w":16,"h":16,"assets":[{assets}],"layers":[]}}"#
            );
            let result = RasterDotLottieAdapter.prepare(
                DotLottieAsset::new(archive(
                    r#"{"animations":[{"id":"main"}]}"#,
                    &too_many_images,
                )),
                DotLottieLimits::strict(),
            );
            assert_eq!(
                result.expect_err("image count must be rejected").category(),
                DotLottieErrorKind::ImageCount
            );
        }
    }
}

#[cfg(feature = "dotlottie")]
pub use raster::RasterDotLottieAdapter;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_recipes_own_slots_timelines_posters_and_rtl() {
        let recipes = [
            CinematicRecipe::Arrival,
            CinematicRecipe::Delegation,
            CinematicRecipe::Handoff,
            CinematicRecipe::Aggregation,
            CinematicRecipe::Success,
            CinematicRecipe::Reward,
            CinematicRecipe::Attention,
            CinematicRecipe::Refusal,
            CinematicRecipe::Failure,
        ];
        for recipe in recipes {
            assert!(!recipe.asset_slot().is_empty());
            assert!(!recipe.duration().is_zero());
            assert!(recipe.poster_progress_per_mille() <= 1_000);
        }
        assert!(CinematicRecipe::Delegation.mirrors_in_rtl());
        assert!(CinematicRecipe::Handoff.mirrors_in_rtl());
        assert!(!CinematicRecipe::Reward.mirrors_in_rtl());
    }

    #[test]
    fn samples_and_state_machine_inputs_refuse_invalid_values() {
        assert_eq!(
            DotLottieSample::at_progress(f32::NAN, false)
                .expect_err("NaN progress must be rejected")
                .category(),
            DotLottieErrorKind::InvalidInput
        );
        assert_eq!(
            DotLottieInput::new("unsafe name", DotLottieInputValue::Trigger)
                .expect_err("unsafe name must be rejected")
                .category(),
            DotLottieErrorKind::InvalidInput
        );
        assert_eq!(
            DotLottieInput::new("speed", DotLottieInputValue::Number(f32::INFINITY))
                .expect_err("infinite input must be rejected")
                .category(),
            DotLottieErrorKind::InvalidInput
        );
        assert_eq!(
            DotLottieInput::new(
                "speed",
                DotLottieInputValue::Number(dotlottie_hard_limits::INPUT_MAGNITUDE + 1.0),
            )
            .expect_err("oversized input must be rejected")
            .category(),
            DotLottieErrorKind::InvalidInput
        );
        assert!(DotLottieInput::new("persona.mood", DotLottieInputValue::Number(0.75)).is_ok());
    }

    #[test]
    fn unavailable_adapter_is_an_explicit_core_fallback() {
        let adapter = UnavailableDotLottieAdapter;
        assert!(!adapter.available());
        let result = adapter.prepare(
            DotLottieAsset::new(Vec::<u8>::new()),
            DotLottieLimits::strict(),
        );
        assert_eq!(
            result
                .expect_err("unavailable adapter must refuse preparation")
                .category(),
            DotLottieErrorKind::RuntimeUnavailable
        );
    }
}
