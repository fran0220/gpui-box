//! Measured label placement shared by normalized visualization families.
use gpui::{Bounds, Point, Size, bounds, point};

/// Try the shape center, then nearby rows. Reject labels wider than the frame
/// or when every measured slot collides; exact values remain in the readout.
pub(super) fn place(
    anchor: Point<f32>,
    extent: Size<f32>,
    frame: Size<f32>,
    occupied: &[Bounds<f32>],
    gap: f32,
) -> Option<Bounds<f32>> {
    if extent.width > frame.width || extent.height > frame.height || extent.height <= 0.0 {
        return None;
    }
    let x = (anchor.x - extent.width / 2.0).clamp(0.0, frame.width - extent.width);
    let y = (anchor.y - extent.height / 2.0).clamp(0.0, frame.height - extent.height);
    let stride = extent.height + gap;
    let rows = (frame.height / stride).ceil() as usize;
    for row in 0..=rows {
        for direction in [-1.0, 1.0] {
            let candidate = bounds(
                point(
                    x,
                    (y + direction * row as f32 * stride).clamp(0.0, frame.height - extent.height),
                ),
                extent,
            );
            if candidate.origin.y >= 0.0
                && candidate.origin.y + extent.height <= frame.height
                && occupied.iter().all(|b| {
                    candidate.origin.x >= b.origin.x + b.size.width + gap
                        || candidate.origin.x + extent.width + gap <= b.origin.x
                        || candidate.origin.y >= b.origin.y + b.size.height + gap
                        || candidate.origin.y + extent.height + gap <= b.origin.y
                })
            {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::size;
    #[test]
    fn measured_labels_never_overlap_or_escape_and_overfull_is_suppressed() {
        let frame = size(90.0, 48.0);
        let a = place(point(80.0, 20.0), size(75.0, 14.0), frame, &[], 3.0).expect("first label");
        let b = place(point(80.0, 20.0), size(70.0, 14.0), frame, &[a], 3.0).expect("second label");
        assert!(!a.intersects(&b));
        assert!((a.origin.y - b.origin.y).abs() >= 17.0);
        assert!(b.origin.x + b.size.width <= frame.width);
        assert!(place(point(20.0, 20.0), size(91.0, 14.0), frame, &[], 3.0).is_none());
    }
}
