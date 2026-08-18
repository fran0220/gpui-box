//! A terminal grid: somebody else's program, drawn.
//!
//! The same posture as the rest of `content`, from the far side of a pipe.
//! Bytes arrive from a process this library did not start and cannot vouch
//! for, so nothing here acts on them: an escape sequence moves the cursor or
//! sets a colour and does nothing else, a title is text to show rather than a
//! window to rename, and a report the program asks for is handed back to the
//! host as bytes instead of being written anywhere.
//!
//! The split is the point. [`Emulator`] is a pure fold — bytes in, grid out,
//! no I/O, no clock — and every escape sequence it understands is testable
//! with a byte string. [`Terminal`] paints a grid and reports keystrokes. What
//! neither of them does is open a pty, spawn a shell, or read an environment
//! variable; a terminal that could do those things would be a product, and
//! this crate is not one. The host owns the process and pumps bytes in.
//!
//! Ported from `crabtalk/bezel` (MIT), on `alacritty_terminal` (Apache-2.0).
//! See `PROVENANCE.md`.

mod emulator;
mod input;
mod palette;
mod view;

pub use emulator::{
    CellColor, CellSide, CellSnapshot, CursorSnapshot, Emulator, GridPoint, GridSize,
    SCROLLBACK_LINES, SelectionKind,
};
pub use input::{
    COALESCE_MS, CellHit, InputCoalescer, RESIZE_DEBOUNCE_MS, SELECTION_DRAG_THRESHOLD, cell_at,
    control_bytes, keystroke_bytes, paste_bytes,
};
pub use view::{GridGeometry, GridSnapshot, Terminal, TerminalEvent, TerminalState};
