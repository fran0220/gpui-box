//! Test helpers that assert native UI behavior and capture product-owned windows.

pub mod audit;
pub mod capture;
#[cfg(feature = "test-support")]
pub mod harness;
pub mod performance;

pub use audit::{AuditError, Finding, Problem, audit, audit_or_error};
pub use performance::{
    PerformanceBudget, PerformanceBudgetError, PerformanceLimit, PerformanceMetric,
    PerformanceReport, PerformanceSample, PerformanceViolation,
};

use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use gpui_kit_semantics::{Node, Snapshot};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SemanticAssertionError {
    #[error("semantic node `{0}` is missing")]
    Missing(String),
    #[error("semantic node `{0}` is not visible")]
    Invisible(String),
    #[error("semantic node `{0}` is disabled")]
    Disabled(String),
    #[error("semantic node `{id}` has text {actual:?}, expected {expected:?}")]
    Text {
        id: String,
        expected: String,
        actual: Option<String>,
    },
}

pub fn present<'a>(snapshot: &'a Snapshot, id: &str) -> Result<&'a Node, SemanticAssertionError> {
    snapshot
        .find(id)
        .ok_or_else(|| SemanticAssertionError::Missing(id.into()))
}

pub fn visible<'a>(snapshot: &'a Snapshot, id: &str) -> Result<&'a Node, SemanticAssertionError> {
    let node = present(snapshot, id)?;
    if node.visible {
        Ok(node)
    } else {
        Err(SemanticAssertionError::Invisible(id.into()))
    }
}

pub fn actionable<'a>(
    snapshot: &'a Snapshot,
    id: &str,
) -> Result<&'a Node, SemanticAssertionError> {
    let node = visible(snapshot, id)?;
    if node.disabled {
        Err(SemanticAssertionError::Disabled(id.into()))
    } else {
        Ok(node)
    }
}

pub fn text<'a>(
    snapshot: &'a Snapshot,
    id: &str,
    expected: &str,
) -> Result<&'a Node, SemanticAssertionError> {
    let node = visible(snapshot, id)?;
    if node.text.as_deref() == Some(expected) {
        Ok(node)
    } else {
        Err(SemanticAssertionError::Text {
            id: id.into(),
            expected: expected.into(),
            actual: node.text.as_ref().map(ToString::to_string),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameDiff {
    pub changed_pixels: usize,
    pub changed_ratio: f32,
    pub max_channel_delta: u8,
    pub mean_channel_delta: f32,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FrameDiffError {
    #[error("frame dimensions differ: {left_width}×{left_height} vs {right_width}×{right_height}")]
    Dimensions {
        left_width: u32,
        left_height: u32,
        right_width: u32,
        right_height: u32,
    },
    #[error("frame byte length does not match RGBA dimensions")]
    InvalidLength,
}

pub fn compare_frames(
    left: &capture::Frame,
    right: &capture::Frame,
    channel_tolerance: u8,
) -> Result<FrameDiff, FrameDiffError> {
    if left.width != right.width || left.height != right.height {
        return Err(FrameDiffError::Dimensions {
            left_width: left.width,
            left_height: left.height,
            right_width: right.width,
            right_height: right.height,
        });
    }
    let expected = left.width as usize * left.height as usize * 4;
    if left.rgba.len() != expected || right.rgba.len() != expected {
        return Err(FrameDiffError::InvalidLength);
    }

    let mut changed_pixels = 0;
    let mut max_channel_delta = 0;
    let mut sum = 0_u64;
    for (left_pixel, right_pixel) in left.rgba.chunks_exact(4).zip(right.rgba.chunks_exact(4)) {
        let mut changed = false;
        for channel in 0..4 {
            let delta = left_pixel[channel].abs_diff(right_pixel[channel]);
            max_channel_delta = max_channel_delta.max(delta);
            sum += u64::from(delta);
            changed |= delta > channel_tolerance;
        }
        changed_pixels += usize::from(changed);
    }
    let pixels = left.width as usize * left.height as usize;
    Ok(FrameDiff {
        changed_pixels,
        changed_ratio: changed_pixels as f32 / pixels.max(1) as f32,
        max_channel_delta,
        mean_channel_delta: sum as f32 / expected.max(1) as f32,
    })
}

/// A directory of PNG visual baselines addressed by stable fixture names.
///
/// The caller owns the directory and chooses its platform boundary. Keep
/// macOS and Windows in separate directories because native text and edge
/// rasterization are platform output, not noise to hide with one tolerance.
#[derive(Debug, Clone)]
pub struct VisualBaselines {
    directory: PathBuf,
    channel_tolerance: u8,
}

impl VisualBaselines {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            channel_tolerance: 0,
        }
    }

    /// Allows a per-channel delta no larger than `tolerance`.
    ///
    /// Exact comparison is the default. Raise this only for a native renderer
    /// known to move antialiased edges by a bounded amount on supported hosts.
    pub fn with_channel_tolerance(mut self, tolerance: u8) -> Self {
        self.channel_tolerance = tolerance;
        self
    }

    /// Writes or replaces one explicitly named baseline.
    pub fn capture(&self, name: &str, frame: &capture::Frame) -> Result<PathBuf, BaselineError> {
        let path = self.path(name)?;
        frame
            .write_png(&path)
            .map_err(|source| BaselineError::Write {
                path: path.clone(),
                source,
            })?;
        Ok(path)
    }

    /// Compares a frame with its named baseline.
    ///
    /// A successful result includes the measured (possibly tolerated) diff.
    /// A failure names the baseline and reports dimensions or pixel-distance
    /// metrics so a test log says what changed before images are inspected.
    pub fn check(&self, name: &str, frame: &capture::Frame) -> Result<FrameDiff, BaselineError> {
        let path = self.path(name)?;
        if !path.exists() {
            return Err(BaselineError::Missing {
                name: name.into(),
                path,
            });
        }
        let baseline = read_png(&path, frame)?;
        let diff = compare_frames(&baseline, frame, self.channel_tolerance).map_err(|source| {
            BaselineError::Compare {
                name: name.into(),
                path: path.clone(),
                source,
            }
        })?;
        if diff.changed_pixels > 0 {
            return Err(BaselineError::Mismatch {
                name: name.into(),
                path,
                diff,
            });
        }
        Ok(diff)
    }

    fn path(&self, name: &str) -> Result<PathBuf, BaselineError> {
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(BaselineError::InvalidName(name.into()));
        }
        Ok(self.directory.join(format!("{name}.png")))
    }
}

#[derive(Debug, Error)]
pub enum BaselineError {
    #[error(
        "visual baseline name `{0}` is invalid; use only ASCII letters, digits, `.`, `_`, and `-`"
    )]
    InvalidName(String),
    #[error(
        "visual baseline `{name}` is missing at {}; capture it before checking",
        path.display()
    )]
    Missing { name: String, path: PathBuf },
    #[error("could not read visual baseline at {}: {source}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not decode visual baseline at {}: {message}", path.display())]
    Decode { path: PathBuf, message: String },
    #[error("could not write visual baseline at {}: {source}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: capture::CaptureError,
    },
    #[error("could not compare visual baseline `{name}` at {}: {source}", path.display())]
    Compare {
        name: String,
        path: PathBuf,
        #[source]
        source: FrameDiffError,
    },
    #[error(
        "visual baseline `{name}` at {} changed: pixel count {} ({:.4}%), maximum channel delta {}, mean channel delta {:.4}; capture this name to accept the new frame",
        path.display(),
        diff.changed_pixels,
        diff.changed_ratio * 100.0,
        diff.max_channel_delta,
        diff.mean_channel_delta
    )]
    Mismatch {
        name: String,
        path: PathBuf,
        diff: FrameDiff,
    },
}

fn read_png(path: &Path, actual: &capture::Frame) -> Result<capture::Frame, BaselineError> {
    let file = fs::File::open(path).map_err(|source| BaselineError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let decoder = png::Decoder::new(BufReader::new(file));
    let mut reader = decoder.read_info().map_err(|error| BaselineError::Decode {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| BaselineError::Decode {
            path: path.to_path_buf(),
            message: "decoded image is too large for this machine".into(),
        })?;
    let mut rgba = vec![0; size];
    let info = reader
        .next_frame(&mut rgba)
        .map_err(|error| BaselineError::Decode {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(BaselineError::Decode {
            path: path.to_path_buf(),
            message: format!(
                "expected RGBA8, found {:?} {:?}",
                info.color_type, info.bit_depth
            ),
        });
    }
    rgba.truncate(info.buffer_size());
    Ok(capture::Frame {
        width: info.width,
        height: info.height,
        scale_factor: actual.scale_factor,
        content_width: actual.content_width,
        content_height: actual.content_height,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use gpui_kit_semantics::{Rect, Role};

    use super::*;

    fn node(id: &str, disabled: bool) -> Node {
        Node {
            id: id.into(),
            role: Role::Button,
            parent: None,
            text: Some("Run".into()),
            bounds: Rect {
                x: 0.0,
                y: 0.0,
                width: 20.0,
                height: 10.0,
            },
            visible: true,
            disabled,
            ..Node::default()
        }
    }

    #[test]
    fn actionable_rejects_disabled_nodes() {
        let snapshot = Snapshot {
            generation: 1,
            nodes: vec![node("run", true)],
        };
        assert_eq!(
            actionable(&snapshot, "run").expect_err("disabled"),
            SemanticAssertionError::Disabled("run".into())
        );
    }

    #[test]
    fn frame_diff_counts_pixels_not_channels() {
        let frame = |rgba| capture::Frame {
            width: 2,
            height: 1,
            scale_factor: 1.0,
            content_width: 2.0,
            content_height: 1.0,
            rgba,
        };
        let left = frame(vec![0, 0, 0, 255, 1, 1, 1, 255]);
        let right = frame(vec![2, 2, 2, 255, 1, 1, 1, 255]);
        let diff = compare_frames(&left, &right, 1).expect("same dimensions");
        assert_eq!(diff.changed_pixels, 1);
        assert_eq!(diff.changed_ratio, 0.5);
        assert_eq!(diff.max_channel_delta, 2);
    }

    #[test]
    fn named_visual_baselines_capture_check_and_diagnose_changes() {
        let directory = tempfile::tempdir().expect("temporary baselines");
        let baselines = VisualBaselines::new(directory.path());
        let frame = capture::Frame {
            width: 2,
            height: 1,
            scale_factor: 2.0,
            content_width: 1.0,
            content_height: 0.5,
            rgba: vec![0, 0, 0, 255, 10, 10, 10, 255],
        };

        let path = baselines.capture("gallery-dark", &frame).expect("capture");
        assert_eq!(path, directory.path().join("gallery-dark.png"));
        assert_eq!(
            baselines.check("gallery-dark", &frame).expect("matches"),
            FrameDiff {
                changed_pixels: 0,
                changed_ratio: 0.0,
                max_channel_delta: 0,
                mean_channel_delta: 0.0,
            }
        );

        let mut changed = frame.clone();
        changed.rgba[0] = 4;
        let message = baselines
            .check("gallery-dark", &changed)
            .expect_err("changed pixel")
            .to_string();
        assert!(message.contains("visual baseline `gallery-dark`"));
        assert!(message.contains("pixel count 1 (50.0000%)"));
        assert!(message.contains("maximum channel delta 4"));
    }

    #[test]
    fn named_visual_baselines_reject_paths_and_report_missing_names() {
        let directory = tempfile::tempdir().expect("temporary baselines");
        let baselines = VisualBaselines::new(directory.path());
        let frame = capture::Frame {
            width: 1,
            height: 1,
            scale_factor: 1.0,
            content_width: 1.0,
            content_height: 1.0,
            rgba: vec![0, 0, 0, 255],
        };

        assert!(matches!(
            baselines.check("../outside", &frame),
            Err(BaselineError::InvalidName(_))
        ));
        let message = baselines
            .check("not-captured", &frame)
            .expect_err("missing")
            .to_string();
        assert!(message.contains("visual baseline `not-captured` is missing"));
        assert!(message.contains("capture it before checking"));
    }
}
