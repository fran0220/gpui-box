//! Window-level frame grabs, taken inside the process that owns the window.
//!
//! Deliberately not `screencapture(1)`. A full-screen OS capture is the wrong
//! tool three times over: on macOS it raises a TCC consent sheet that lands on
//! top of the very window under test, it returns the whole desktop so every
//! assertion needs a crop the test cannot compute, and it captures whatever
//! happens to be in front of the window rather than the window. Asking the
//! window server for one window id, from inside the app that owns it, has none
//! of those problems.
//!
//! The captured rectangle is the window's **content area**, so image pixels map
//! onto semantic-tree coordinates by `scale_factor` alone: a verifier that
//! knows an element's bounds knows which pixels to sample.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

/// One grabbed frame, in straight RGBA8.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// Device pixels.
    pub width: u32,
    pub height: u32,
    /// Device pixels per logical pixel, as the window reported it.
    pub scale_factor: f32,
    /// Logical content rectangle the crop was taken from (title bar excluded).
    pub content_width: f32,
    pub content_height: f32,
    pub rgba: Vec<u8>,
}

impl Frame {
    /// Writes the frame as a PNG and reports the file size.
    pub fn write_png(&self, path: &Path) -> Result<u64, CaptureError> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let file = File::create(path)?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| CaptureError::Encode(error.to_string()))?;
        writer
            .write_image_data(&self.rgba)
            .map_err(|error| CaptureError::Encode(error.to_string()))?;
        writer
            .finish()
            .map_err(|error| CaptureError::Encode(error.to_string()))?;
        Ok(std::fs::metadata(path)?.len())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("the window could not be identified to the window server: {0}")]
    Window(String),
    #[error(
        "the window server returned no image for this window; it may be minimised, off-screen, \
         or hidden behind a consent prompt"
    )]
    NoImage,
    #[error("the window server returned a frame in an unsupported pixel format: {0}")]
    PixelFormat(String),
    #[error("the screenshot could not be written: {0}")]
    Io(#[from] std::io::Error),
    #[error("the screenshot could not be encoded: {0}")]
    Encode(String),
    #[error("in-process window capture is implemented for macOS only")]
    Unsupported,
}

/// Grabs the content area of `window`.
pub fn capture_window(window: &gpui::Window) -> Result<Frame, CaptureError> {
    platform::capture(window)
}

#[cfg(target_os = "macos")]
mod platform {
    use core_graphics::geometry::{CGPoint, CGRect, CGSize};
    use core_graphics::window::{
        create_image, kCGWindowImageBestResolution, kCGWindowImageBoundsIgnoreFraming,
        kCGWindowListOptionIncludingWindow,
    };
    use objc2_app_kit::NSView;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    use super::{CaptureError, Frame};

    /// The frame rectangle as the window server knows it, plus the content
    /// rectangle inside it that the shell actually draws into.
    struct Geometry {
        window_id: u32,
        /// Insets of the content area within the frame, in logical pixels,
        /// measured from the top left.
        left_inset: f64,
        top_inset: f64,
        frame_width: f64,
        content_width: f64,
        content_height: f64,
        scale_factor: f64,
    }

    fn geometry(window: &gpui::Window) -> Result<Geometry, CaptureError> {
        // Spelled out: `gpui::Window` has an inherent `window_handle` of its own
        // that answers with gpui's handle, not the platform's.
        let handle = HasWindowHandle::window_handle(window)
            .map_err(|error| CaptureError::Window(error.to_string()))?;
        let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
            return Err(CaptureError::Window(
                "this window is not an AppKit window".into(),
            ));
        };

        // The only unsafe step in the crate: the platform handed us a pointer to
        // a live `NSView` it owns, and there is no safe way to say so. Nothing
        // is constructed, freed or mutated through it — the borrow ends inside
        // this function.
        #[allow(unsafe_code)]
        let view: &NSView = unsafe { appkit.ns_view.cast::<NSView>().as_ref() };
        let ns_window = view
            .window()
            .ok_or_else(|| CaptureError::Window("the view is not in a window".into()))?;

        let frame = ns_window.frame();
        let content = ns_window.contentRectForFrameRect(frame);
        let bottom_inset = content.origin.y - frame.origin.y;
        Ok(Geometry {
            window_id: ns_window.windowNumber() as u32,
            left_inset: content.origin.x - frame.origin.x,
            top_inset: frame.size.height - content.size.height - bottom_inset,
            frame_width: frame.size.width,
            content_width: content.size.width,
            content_height: content.size.height,
            scale_factor: ns_window.backingScaleFactor(),
        })
    }

    pub(super) fn capture(window: &gpui::Window) -> Result<Frame, CaptureError> {
        let geometry = geometry(window)?;

        // A null rectangle means "whatever this window occupies"; ignoring the
        // framing drops the drop shadow, which would otherwise put transparent
        // pixels around every sample a verifier takes.
        let null_rect = CGRect::new(
            &CGPoint::new(f64::INFINITY, f64::INFINITY),
            &CGSize::new(0.0, 0.0),
        );
        let image = create_image(
            null_rect,
            kCGWindowListOptionIncludingWindow,
            geometry.window_id,
            kCGWindowImageBoundsIgnoreFraming | kCGWindowImageBestResolution,
        )
        .ok_or(CaptureError::NoImage)?;

        if image.bits_per_pixel() != 32 || image.bits_per_component() != 8 {
            return Err(CaptureError::PixelFormat(format!(
                "{} bits per pixel, {} per component",
                image.bits_per_pixel(),
                image.bits_per_component()
            )));
        }

        let source_width = image.width() as u32;
        let source_height = image.height() as u32;
        if source_width == 0 || source_height == 0 {
            return Err(CaptureError::NoImage);
        }
        // Derived from the image rather than trusted from the display: a window
        // straddling two displays is captured at one of their scales, and the
        // crop has to use the scale that was actually applied.
        let scale = if geometry.frame_width > 0.0 {
            f64::from(source_width) / geometry.frame_width
        } else {
            geometry.scale_factor
        };

        let left = (geometry.left_inset * scale).round().max(0.0) as u32;
        let top = (geometry.top_inset * scale).round().max(0.0) as u32;
        let width = ((geometry.content_width * scale).round() as u32).min(source_width - left);
        let height = ((geometry.content_height * scale).round() as u32).min(source_height - top);
        if width == 0 || height == 0 {
            return Err(CaptureError::NoImage);
        }

        let stride = image.bytes_per_row();
        let data = image.data();
        let source = data.bytes();
        let mut rgba = Vec::with_capacity((width as usize) * (height as usize) * 4);
        for row in 0..height {
            let start = (top + row) as usize * stride + (left as usize) * 4;
            let line = &source[start..start + (width as usize) * 4];
            // The window server hands back premultiplied BGRA; PNG wants RGBA.
            // The alpha of an opaque window is 255, so unpremultiplying would
            // be a no-op division.
            for pixel in line.chunks_exact(4) {
                rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
            }
        }

        Ok(Frame {
            width,
            height,
            scale_factor: scale as f32,
            content_width: geometry.content_width as f32,
            content_height: geometry.content_height as f32,
            rgba,
        })
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::{CaptureError, Frame};

    pub(super) fn capture(_window: &gpui::Window) -> Result<Frame, CaptureError> {
        Err(CaptureError::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_round_trips_through_png_at_its_own_dimensions() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("nested/frame.png");
        let frame = Frame {
            width: 3,
            height: 2,
            scale_factor: 2.0,
            content_width: 1.5,
            content_height: 1.0,
            rgba: vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 1, 2, 3, 255, 4, 5, 6, 255, 7, 8,
                9, 255,
            ],
        };

        let bytes = frame.write_png(&path).expect("write PNG");
        assert!(bytes > 0, "the screenshot file is empty");

        let decoder = png::Decoder::new(std::io::BufReader::new(
            File::open(&path).expect("open PNG"),
        ));
        let mut reader = decoder.read_info().expect("read PNG header");
        let mut buffer = vec![0; reader.output_buffer_size().expect("bounded PNG output")];
        let info = reader.next_frame(&mut buffer).expect("decode PNG");
        assert_eq!((info.width, info.height), (3, 2));
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert_eq!(&buffer[..info.buffer_size()], frame.rgba.as_slice());
    }
}
