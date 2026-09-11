//! Raw UIKit ABI. All functions require the main thread. The callback context
//! for `gpui_ios_run` has process lifetime; window callbacks must remain valid
//! until destruction finishes. No Rust panic may cross the native boundary.
//!
//! Raw view/controller pointers are borrowed from the host. Drop every renderer
//! and surface referencing them *before* destroying the host. A native handle
//! is not permission to access UIKit from a render worker.

use std::ffi::{c_char, c_int, c_void};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Edges {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct TextRange {
    pub location: usize,
    pub length: usize,
    pub valid: bool,
}

impl TextRange {
    pub fn into_range(self) -> Option<std::ops::Range<usize>> {
        self.valid
            .then_some(self.location..self.location.checked_add(self.length)?)
    }
}

impl From<std::ops::Range<usize>> for TextRange {
    fn from(range: std::ops::Range<usize>) -> Self {
        match range.end.checked_sub(range.start) {
            Some(length) => Self {
                location: range.start,
                length,
                valid: true,
            },
            None => Self::default(),
        }
    }
}

/// All text callbacks are required for an editable host, including an empty
/// document. `text` returns required UTF-8 bytes; a zero-capacity call is a
/// length query. `mark`'s selection is relative to the new marked text.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Callbacks {
    pub context: *mut c_void,
    pub launched: Option<unsafe extern "C" fn(*mut c_void)>,
    pub lifecycle: Option<unsafe extern "C" fn(*mut c_void, u32)>,
    pub memory_warning: Option<unsafe extern "C" fn(*mut c_void)>,
    pub frame: Option<unsafe extern "C" fn(*mut c_void, f64)>,
    pub geometry: Option<unsafe extern "C" fn(*mut c_void, Rect, f64, Edges, Edges)>,
    pub touch: Option<unsafe extern "C" fn(*mut c_void, u64, u32, f64, f64, f64)>,
    pub cancel_input: Option<unsafe extern "C" fn(*mut c_void)>,
    pub text: unsafe extern "C" fn(*mut c_void, TextRange, *mut c_char, usize) -> usize,
    pub text_length: unsafe extern "C" fn(*mut c_void) -> usize,
    pub selection: unsafe extern "C" fn(*mut c_void) -> TextRange,
    pub marked: unsafe extern "C" fn(*mut c_void) -> TextRange,
    pub select: unsafe extern "C" fn(*mut c_void, TextRange),
    pub replace: unsafe extern "C" fn(*mut c_void, TextRange, *const c_char, usize),
    pub mark: unsafe extern "C" fn(*mut c_void, *const c_char, usize, TextRange),
    pub unmark: unsafe extern "C" fn(*mut c_void),
    pub text_bounds: unsafe extern "C" fn(*mut c_void, TextRange) -> Rect,
    pub text_hit_test: unsafe extern "C" fn(*mut c_void, f64, f64) -> usize,
    pub accessibility_action: Option<unsafe extern "C" fn(*mut c_void, u64, u32) -> bool>,
    pub system_event: Option<unsafe extern "C" fn(*mut c_void, u32, *const c_char)>,
    pub text_action: Option<unsafe extern "C" fn(*mut c_void, u32) -> bool>,
    pub native_selection: Option<unsafe extern "C" fn(*mut c_void) -> NativeSelection>,
    pub set_native_selection: Option<unsafe extern "C" fn(*mut c_void, NativeSelection) -> bool>,
    pub move_position:
        Option<unsafe extern "C" fn(*mut c_void, NativePosition, u32, usize) -> NativePosition>,
    pub farthest_position:
        Option<unsafe extern "C" fn(*mut c_void, TextRange, u32) -> NativePosition>,
    pub position_bounds:
        Option<unsafe extern "C" fn(*mut c_void, NativePosition, *mut Rect) -> bool>,
    pub selection_rects:
        Option<unsafe extern "C" fn(*mut c_void, TextRange, *mut SelectionRect, usize) -> usize>,
    pub grapheme: Option<unsafe extern "C" fn(*mut c_void, usize) -> TextRange>,
    pub writing_direction: Option<unsafe extern "C" fn(*mut c_void, usize) -> i32>,
    pub set_writing_direction: Option<unsafe extern "C" fn(*mut c_void, TextRange, bool) -> bool>,
    pub position_for_point:
        Option<unsafe extern "C" fn(*mut c_void, f64, f64, TextRange) -> NativePosition>,
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct NativePosition {
    pub offset: usize,
    pub upstream: bool,
    pub valid: bool,
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct NativeSelection {
    pub anchor: NativePosition,
    pub head: NativePosition,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct SelectionRect {
    pub bounds: Rect,
    pub rtl: bool,
    pub contains_start: bool,
    pub contains_end: bool,
    pub vertical: bool,
}

#[repr(C)]
pub struct AccessibilityNode {
    pub id: u64,
    pub bounds: Rect,
    pub label: *const c_char,
    pub value: *const c_char,
    pub role: u32,
    pub disabled: bool,
    pub selected: bool,
}

unsafe extern "C" {
    pub fn gpui_ios_run(callbacks: Callbacks) -> c_int;
    pub fn gpui_ios_create(callbacks: Callbacks) -> *mut c_void;
    pub fn gpui_ios_destroy(host: *mut c_void);
    pub fn gpui_ios_view(host: *mut c_void) -> *mut c_void;
    pub fn gpui_ios_controller(host: *mut c_void) -> *mut c_void;
    pub fn gpui_ios_keyboard(host: *mut c_void, visible: bool);
    pub fn gpui_ios_text_changed(host: *mut c_void, selection_only: bool);
    pub fn gpui_ios_set_title(host: *mut c_void, title: *const c_char);
    pub fn gpui_ios_accessibility(host: *mut c_void, nodes: *const AccessibilityNode, count: usize);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_are_utf16_offsets_not_utf8_bytes() {
        let source = "a😀中";
        let range = TextRange::from(1..3);
        let utf16 = source.encode_utf16().collect::<Vec<_>>();
        assert_eq!(
            String::from_utf16(&utf16[range.into_range().expect("valid range")])
                .expect("complete surrogate pair"),
            "😀"
        );
        assert_eq!(TextRange::from(4..4).into_range(), Some(4..4));
        assert_eq!(TextRange::default().into_range(), None);
        assert_eq!(
            TextRange {
                location: usize::MAX,
                length: 1,
                valid: true
            }
            .into_range(),
            None
        );
    }
}
