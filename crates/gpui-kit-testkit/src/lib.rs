//! Test helpers that assert native UI behavior and capture product-owned windows.

pub mod audit;
pub mod capture;
#[cfg(feature = "test-support")]
pub mod harness;

pub use audit::{Finding, Problem, audit};

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
            actual: node.text.clone(),
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
}
