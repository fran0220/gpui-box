//! Keystrokes, pastes and pointer positions, turned into the bytes and cells a
//! terminal speaks in.
//!
//! Ported from `crabtalk/bezel` (MIT); see `PROVENANCE.md`.
//!
//! Every function here is pure. That is deliberate: the encoding of a keystroke
//! is the part of a terminal most likely to be subtly wrong for one program on
//! one platform, and a pure function is the part that can be pinned down with a
//! byte string instead of a running shell.

use gpui::Modifiers;

use super::emulator::CellSide;

/// How long keyboard bytes are held before the host is told to write them.
///
/// A held key repeats faster than a frame, and a write per repeat turns a
/// scrolled page into a write storm. The timer belongs to the host; the buffer
/// under it is [`InputCoalescer`], which is pure.
pub const COALESCE_MS: u64 = 12;

/// How long a measured grid size waits before the host is told to resize.
///
/// A drag across a window edge produces a size per frame, and a program that
/// redraws on `SIGWINCH` redraws for each one. Waiting for the drag to settle
/// costs a moment of stale wrapping and saves the redraw storm.
pub const RESIZE_DEBOUNCE_MS: u64 = 80;

/// How far a pointer travels before a press becomes a selection.
///
/// Without a threshold, the click that focuses the panel starts a one-cell
/// selection whenever the hand moves a pixel, and a host that copies on
/// selection change then clobbers the clipboard on every click.
pub const SELECTION_DRAG_THRESHOLD: f32 = 2.0;

/// Which cell a pointer landed on, and which edge of it an anchor takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellHit {
    pub row: usize,
    pub col: usize,
    pub side: CellSide,
}

/// Map a position measured from the grid's first glyph onto a cell.
///
/// A position outside the grid clamps to the nearest cell rather than reporting
/// nothing, because that is what a drag needs: the pointer routinely leaves the
/// panel mid-gesture, and the selection should reach the edge it left through
/// instead of freezing at the last sample taken inside.
///
/// Overshoot also *forces the side*, which clamping alone does not do. Dragging
/// past the bottom should take the last row whole even when the pointer drifted
/// left of where it started, and deriving the side from x there would stop the
/// selection mid-row.
pub fn cell_at(
    x: f32,
    y: f32,
    cell_width: f32,
    line_height: f32,
    cols: usize,
    rows: usize,
) -> CellHit {
    let origin = CellHit {
        row: 0,
        col: 0,
        side: CellSide::Left,
    };
    // A collapsed grid or a font probe that returned NaN would divide into
    // garbage indices, and a garbage index is a panic one layer up.
    let usable = |value: f32| value.is_finite() && value > 0.0;
    if cols == 0 || rows == 0 || !usable(cell_width) || !usable(line_height) {
        return origin;
    }
    let x = if x.is_finite() { x } else { 0.0 };
    let y = if y.is_finite() { y } else { 0.0 };
    let last_col = cols - 1;
    let last_row = rows - 1;

    let raw_col = (x / cell_width).floor();
    let mut side = if x.max(0.0) % cell_width > cell_width / 2.0 {
        CellSide::Right
    } else {
        CellSide::Left
    };
    let col = if raw_col > last_col as f32 {
        side = CellSide::Right;
        last_col
    } else {
        raw_col.max(0.0) as usize
    };

    let raw_row = (y / line_height).floor();
    let row = if raw_row > last_row as f32 {
        side = CellSide::Right;
        last_row
    } else if raw_row < 0.0 {
        side = CellSide::Left;
        0
    } else {
        raw_row as usize
    };

    CellHit { row, col, side }
}

/// Encode a keystroke as terminal bytes, or report that it is not the
/// terminal's.
///
/// `None` is not a failure: it is how a keystroke falls through to the
/// application keymap. A platform-primary combination is always the
/// application's, because a terminal that swallowed it would take the window's
/// close and quit with it.
///
/// `app_cursor` is DECCKM, and it is why this needs the emulator's mode rather
/// than a fixed table: the same arrow key is `ESC [ A` normally and `ESC O A`
/// once a full-screen program has asked for application cursor keys.
pub fn keystroke_bytes(
    key: &str,
    key_char: Option<&str>,
    modifiers: &Modifiers,
    app_cursor: bool,
) -> Option<Vec<u8>> {
    if modifiers.platform {
        return None;
    }
    if modifiers.alt {
        // Alt is an ESC prefix on the same keystroke without it, which is what
        // every readline binding expects.
        let inner = keystroke_bytes(
            key,
            key_char,
            &Modifiers {
                alt: false,
                ..*modifiers
            },
            app_cursor,
        )?;
        let mut bytes = vec![0x1b];
        bytes.extend(inner);
        return Some(bytes);
    }
    if modifiers.control {
        return control_bytes(key);
    }

    let cursor_key = |csi: &[u8], ss3: &[u8]| {
        Some(if app_cursor {
            ss3.to_vec()
        } else {
            csi.to_vec()
        })
    };
    match key {
        "enter" => Some(b"\r".to_vec()),
        "backspace" => Some(vec![0x7f]),
        "tab" => Some(if modifiers.shift {
            b"\x1b[Z".to_vec()
        } else {
            b"\t".to_vec()
        }),
        "escape" => Some(vec![0x1b]),
        "space" => Some(b" ".to_vec()),
        "up" => cursor_key(b"\x1b[A", b"\x1bOA"),
        "down" => cursor_key(b"\x1b[B", b"\x1bOB"),
        "right" => cursor_key(b"\x1b[C", b"\x1bOC"),
        "left" => cursor_key(b"\x1b[D", b"\x1bOD"),
        "home" => cursor_key(b"\x1b[H", b"\x1bOH"),
        "end" => cursor_key(b"\x1b[F", b"\x1bOF"),
        "insert" => Some(b"\x1b[2~".to_vec()),
        "delete" => Some(b"\x1b[3~".to_vec()),
        "pageup" => Some(b"\x1b[5~".to_vec()),
        "pagedown" => Some(b"\x1b[6~".to_vec()),
        "f1" => Some(b"\x1bOP".to_vec()),
        "f2" => Some(b"\x1bOQ".to_vec()),
        "f3" => Some(b"\x1bOR".to_vec()),
        "f4" => Some(b"\x1bOS".to_vec()),
        "f5" => Some(b"\x1b[15~".to_vec()),
        "f6" => Some(b"\x1b[17~".to_vec()),
        "f7" => Some(b"\x1b[18~".to_vec()),
        "f8" => Some(b"\x1b[19~".to_vec()),
        "f9" => Some(b"\x1b[20~".to_vec()),
        "f10" => Some(b"\x1b[21~".to_vec()),
        "f11" => Some(b"\x1b[23~".to_vec()),
        "f12" => Some(b"\x1b[24~".to_vec()),
        _ => {
            // The typed character first, because it is the one that knows about
            // shift, dead keys and every non-US layout. The key name is the
            // fallback for the single-character keys that arrive without one.
            let text = key_char.filter(|text| !text.is_empty()).or({
                if key.chars().count() == 1 {
                    Some(key)
                } else {
                    None
                }
            })?;
            Some(text.as_bytes().to_vec())
        }
    }
}

/// Encode a Ctrl combination in caret notation.
///
/// Public because a host with its own keymap has to be able to ask what
/// `ctrl-c` is without going through a whole `Modifiers`.
pub fn control_bytes(key: &str) -> Option<Vec<u8>> {
    let mut chars = key.chars();
    let (first, rest) = (chars.next()?, chars.next());
    if rest.is_some() {
        return match key {
            "space" => Some(vec![0x00]),
            "backspace" => Some(vec![0x08]),
            "enter" => Some(b"\r".to_vec()),
            _ => None,
        };
    }
    match first {
        'a'..='z' => Some(vec![first as u8 - b'a' + 1]),
        '@' => Some(vec![0x00]),
        '[' => Some(vec![0x1b]),
        '\\' => Some(vec![0x1c]),
        ']' => Some(vec![0x1d]),
        '^' => Some(vec![0x1e]),
        '_' | '/' => Some(vec![0x1f]),
        '?' => Some(vec![0x7f]),
        _ => None,
    }
}

/// Wrap pasted text for the terminal.
///
/// The end marker is stripped whatever the mode, and that is a safety property
/// rather than tidiness: text containing `ESC [201~` would otherwise end the
/// bracket early and hand the rest of itself to the shell as typed input, which
/// is how a copied block of text runs a command nobody typed.
pub fn paste_bytes(text: &str, bracketed: bool) -> Vec<u8> {
    let sanitized = text.replace("\x1b[201~", "");
    if bracketed {
        let mut bytes = b"\x1b[200~".to_vec();
        bytes.extend_from_slice(sanitized.as_bytes());
        bytes.extend_from_slice(b"\x1b[201~");
        bytes
    } else {
        sanitized.into_bytes()
    }
}

/// Holds keyboard bytes between flushes.
///
/// [`Self::push`] reports `true` exactly when a flush should be scheduled — the
/// buffer having been empty — so a burst of repeats schedules one timer rather
/// than one per key.
#[derive(Debug, Default)]
pub struct InputCoalescer {
    pending: Vec<u8>,
}

impl InputCoalescer {
    /// Add bytes, reporting whether a flush now needs scheduling.
    pub fn push(&mut self, bytes: &[u8]) -> bool {
        let was_empty = self.pending.is_empty();
        self.pending.extend_from_slice(bytes);
        was_empty && !self.pending.is_empty()
    }

    /// Take everything buffered, leaving the coalescer empty.
    pub fn take(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.pending)
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none() -> Modifiers {
        Modifiers::default()
    }

    fn control() -> Modifiers {
        Modifiers {
            control: true,
            ..Modifiers::default()
        }
    }

    fn bytes(key: &str, modifiers: &Modifiers) -> Option<Vec<u8>> {
        keystroke_bytes(key, None, modifiers, false)
    }

    #[test]
    fn a_printable_key_sends_the_character_that_was_typed() {
        assert_eq!(bytes("a", &none()), Some(b"a".to_vec()));
        assert_eq!(
            keystroke_bytes("a", Some("A"), &none(), false),
            Some(b"A".to_vec()),
            "the typed character wins, because it is the one that knows the layout"
        );
        assert_eq!(bytes("space", &none()), Some(b" ".to_vec()));
    }

    #[test]
    fn a_key_name_nobody_can_encode_falls_through() {
        assert_eq!(bytes("capslock", &none()), None);
        assert_eq!(bytes("f13", &none()), None);
    }

    #[test]
    fn the_platform_modifier_always_belongs_to_the_application() {
        let platform = Modifiers {
            platform: true,
            ..Modifiers::default()
        };
        assert_eq!(bytes("c", &platform), None);
        assert_eq!(bytes("v", &platform), None);
        assert_eq!(bytes("enter", &platform), None);
    }

    #[test]
    fn the_control_keys_send_their_control_codes() {
        assert_eq!(bytes("enter", &none()), Some(b"\r".to_vec()));
        assert_eq!(bytes("backspace", &none()), Some(vec![0x7f]));
        assert_eq!(bytes("tab", &none()), Some(b"\t".to_vec()));
        assert_eq!(bytes("escape", &none()), Some(vec![0x1b]));
    }

    #[test]
    fn shift_tab_is_a_back_tab_rather_than_a_tab() {
        let shift = Modifiers {
            shift: true,
            ..Modifiers::default()
        };
        assert_eq!(bytes("tab", &shift), Some(b"\x1b[Z".to_vec()));
    }

    #[test]
    fn application_cursor_mode_switches_the_arrows_to_ss3() {
        for (key, csi, ss3) in [
            ("up", "\x1b[A", "\x1bOA"),
            ("down", "\x1b[B", "\x1bOB"),
            ("right", "\x1b[C", "\x1bOC"),
            ("left", "\x1b[D", "\x1bOD"),
            ("home", "\x1b[H", "\x1bOH"),
            ("end", "\x1b[F", "\x1bOF"),
        ] {
            assert_eq!(
                keystroke_bytes(key, None, &none(), false),
                Some(csi.as_bytes().to_vec())
            );
            assert_eq!(
                keystroke_bytes(key, None, &none(), true),
                Some(ss3.as_bytes().to_vec())
            );
        }
    }

    #[test]
    fn the_navigation_and_function_keys_ignore_cursor_mode() {
        for key in ["insert", "delete", "pageup", "pagedown", "f5", "f12"] {
            assert_eq!(
                keystroke_bytes(key, None, &none(), false),
                keystroke_bytes(key, None, &none(), true)
            );
        }
        assert_eq!(bytes("delete", &none()), Some(b"\x1b[3~".to_vec()));
        assert_eq!(bytes("f1", &none()), Some(b"\x1bOP".to_vec()));
    }

    #[test]
    fn control_letters_are_caret_notation() {
        assert_eq!(bytes("c", &control()), Some(vec![0x03]));
        assert_eq!(bytes("a", &control()), Some(vec![0x01]));
        assert_eq!(bytes("d", &control()), Some(vec![0x04]));
        assert_eq!(bytes("z", &control()), Some(vec![0x1a]));
    }

    #[test]
    fn the_control_punctuation_that_programs_actually_bind() {
        assert_eq!(bytes("space", &control()), Some(vec![0x00]));
        assert_eq!(bytes("[", &control()), Some(vec![0x1b]));
        assert_eq!(bytes("?", &control()), Some(vec![0x7f]));
        assert_eq!(bytes("_", &control()), Some(vec![0x1f]));
        assert_eq!(bytes("1", &control()), None, "not a control code");
    }

    #[test]
    fn alt_prefixes_the_same_keystroke_with_escape() {
        let alt = Modifiers {
            alt: true,
            ..Modifiers::default()
        };
        assert_eq!(bytes("b", &alt), Some(vec![0x1b, b'b']));
        assert_eq!(bytes("enter", &alt), Some(vec![0x1b, b'\r']));

        let alt_control = Modifiers {
            alt: true,
            control: true,
            ..Modifiers::default()
        };
        assert_eq!(bytes("c", &alt_control), Some(vec![0x1b, 0x03]));

        assert_eq!(
            bytes("capslock", &alt),
            None,
            "a key with no encoding does not become a bare escape"
        );
    }

    #[test]
    fn a_plain_paste_is_the_text_it_was() {
        assert_eq!(paste_bytes("hello", false), b"hello".to_vec());
    }

    #[test]
    fn a_bracketed_paste_is_wrapped() {
        assert_eq!(paste_bytes("hi", true), b"\x1b[200~hi\x1b[201~".to_vec(),);
    }

    #[test]
    fn a_paste_cannot_end_its_own_bracket() {
        let hostile = "ls\x1b[201~rm -rf /\n";
        let bracketed = paste_bytes(hostile, true);
        let text = String::from_utf8_lossy(&bracketed);
        assert_eq!(text.matches("\x1b[201~").count(), 1);
        assert!(text.ends_with("\x1b[201~"));
        assert_eq!(
            paste_bytes(hostile, false),
            b"lsrm -rf /\n".to_vec(),
            "the marker is stripped whatever the mode"
        );
    }

    #[test]
    fn the_coalescer_asks_for_one_timer_per_burst() {
        let mut coalescer = InputCoalescer::default();
        assert!(coalescer.is_empty());
        assert!(coalescer.push(b"a"), "the first push schedules a flush");
        assert!(!coalescer.push(b"b"), "the burst is already scheduled");
        assert!(!coalescer.push(b"c"));
        assert_eq!(coalescer.take(), b"abc".to_vec());
        assert!(coalescer.is_empty());
        assert!(coalescer.push(b"d"), "the next burst schedules again");
    }

    #[test]
    fn pushing_nothing_schedules_nothing() {
        let mut coalescer = InputCoalescer::default();
        assert!(!coalescer.push(b""));
        assert!(coalescer.is_empty());
    }

    #[test]
    fn a_pointer_inside_the_grid_lands_on_its_cell() {
        let hit = cell_at(25.0, 36.0, 10.0, 18.0, 80, 24);
        assert_eq!(hit.row, 2);
        assert_eq!(hit.col, 2);
        assert_eq!(hit.side, CellSide::Left, "the left half of the cell");

        let right = cell_at(26.0, 0.0, 10.0, 18.0, 80, 24);
        assert_eq!(right.col, 2);
        assert_eq!(right.side, CellSide::Right);
    }

    #[test]
    fn a_pointer_past_the_grid_clamps_and_takes_the_row_whole() {
        let below = cell_at(3.0, 9_000.0, 10.0, 18.0, 80, 24);
        assert_eq!(below.row, 23);
        assert_eq!(
            below.side,
            CellSide::Right,
            "dragging past the bottom takes the last row whole"
        );

        let above = cell_at(500.0, -50.0, 10.0, 18.0, 80, 24);
        assert_eq!(above.row, 0);
        assert_eq!(above.side, CellSide::Left);

        let right = cell_at(10_000.0, 0.0, 10.0, 18.0, 80, 24);
        assert_eq!(right.col, 79);
        assert_eq!(right.side, CellSide::Right);

        let left = cell_at(-40.0, 18.0, 10.0, 18.0, 80, 24);
        assert_eq!(left.col, 0);
        assert_eq!(left.side, CellSide::Left);
    }

    #[test]
    fn degenerate_metrics_report_the_origin_instead_of_dividing() {
        for hit in [
            cell_at(50.0, 50.0, 0.0, 18.0, 80, 24),
            cell_at(50.0, 50.0, 10.0, 0.0, 80, 24),
            cell_at(50.0, 50.0, f32::NAN, 18.0, 80, 24),
            cell_at(f32::NAN, f32::INFINITY, 10.0, 18.0, 80, 24),
            cell_at(50.0, 50.0, 10.0, 18.0, 0, 24),
            cell_at(50.0, 50.0, 10.0, 18.0, 80, 0),
        ] {
            assert_eq!(hit.row, 0);
            assert_eq!(hit.col, 0);
        }
    }
}
