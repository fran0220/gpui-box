//! Prose and conversation: the two surfaces that draw content nobody in this
//! crate wrote.
//!
//! Both take text from somewhere else — a document, a model, another person —
//! which is what makes them different from the rest of the library. A button's
//! label is written by the application; a Markdown document's contents are
//! not, and may contain a tag, a destination, or an image reference that would
//! act on the reader if anything here acted on it.
//!
//! So nothing here acts. Raw HTML is shown as the characters it is, links
//! report rather than open, images are named rather than fetched, and code
//! changes colour but never shape. `docs/content.md` states the
//! whole posture, and the delivery vocabulary a conversation speaks.
//!
//! The two media surfaces keep the same posture from the other side. Nothing
//! is fetched, so [`ImageViewer`] draws what the host handed it and names what
//! it did not; nothing is played, so [`TransportBar`] reports every control
//! and moves no head. A duration nobody knows is a state, not a zero.
//!
//! Code is the one place the crate reads what it was given rather than only
//! showing it, and [`highlight`] states exactly how far that goes: four
//! classes found by looking, on a language the writer named, changing colour
//! and nothing else.

pub mod agent_document;
pub mod ansi;
pub mod browser;
pub mod code_view;
pub mod diff_view;
pub mod highlight;
pub mod image_viewer;
pub mod log_stream;
pub mod markdown;
pub mod message_list;
pub mod outline;
pub mod rich_text;
#[cfg(all(feature = "terminal", not(target_family = "wasm")))]
pub mod terminal;
pub mod transport;

pub use agent_document::{
    AgentBlockKind, AgentDocument, AgentDocumentBlock, AgentDocumentEvent, AgentDocumentState,
};
pub use ansi::{AnsiRun, strip_ansi};
pub use browser::{BrowserPanel, ViewportState};
pub use code_view::{CodeLine, CodeView, LineMark};
pub use diff_view::{
    DiffCursor, DiffFile, DiffHunk, DiffLine, DiffNote, DiffPresentation, DiffView, DiffViewEvent,
    word_spans,
};
pub use highlight::{Carry, Language};
pub use image_viewer::{FitMode, ImageFrame, ImageSize, ImageState, ImageViewer, ImageViewerEvent};
pub use log_stream::{LogEntry, LogStream, LogStreamState};
pub use markdown::{
    Block, CellAlign, CodeBlock, CodeSpan, Document, ImageRequest, Inline, ListEntry, Markdown,
    MarkdownEvent,
};
pub use message_list::{
    Attachment, DeliveryState, Message, MessageBody, MessageList, Reaction, streaming_since,
};
pub use outline::{Mark, Outline};
pub use rich_text::{
    RichTextAlignment, RichTextBlock, RichTextBlockId, RichTextDocument, RichTextEditResult,
    RichTextEditSession, RichTextError, RichTextFormat, RichTextInlineStyle, RichTextInputKind,
    RichTextIntent, RichTextListItem, RichTextListKind, RichTextParagraphStyle, RichTextPosition,
    RichTextRange, RichTextSelection,
};
#[cfg(all(feature = "terminal", not(target_family = "wasm")))]
pub use terminal::{
    CellHit, CellSide, CellSnapshot, CursorSnapshot, Emulator, GridGeometry, GridPoint, GridSize,
    GridSnapshot, SelectionKind, Terminal, TerminalEvent, TerminalState,
};
pub use transport::{
    BufferedRange, TrackStep, TransportBar, TransportDuration, TransportEvent, TransportState,
};
