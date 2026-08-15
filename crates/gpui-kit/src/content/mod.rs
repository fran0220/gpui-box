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
//! report rather than open, images are named rather than fetched, and code is
//! set plain unless the host supplies the colour. `docs/content.md` states the
//! whole posture, and the delivery vocabulary a conversation speaks.
//!
//! The two media surfaces keep the same posture from the other side. Nothing
//! is fetched, so [`ImageViewer`] draws what the host handed it and names what
//! it did not; nothing is played, so [`TransportBar`] reports every control
//! and moves no head. A duration nobody knows is a state, not a zero.

pub mod agent_document;
pub mod browser;
pub mod code_view;
pub mod diff_view;
pub mod image_viewer;
pub mod log_stream;
pub mod markdown;
pub mod message_list;
pub mod transport;

pub use agent_document::{
    AgentBlockKind, AgentDocument, AgentDocumentBlock, AgentDocumentEvent, AgentDocumentState,
};
pub use browser::{BrowserPanel, ViewportState};
pub use code_view::{CodeLine, CodeView, LineMark};
pub use diff_view::{DiffFile, DiffHunk, DiffLine, DiffPresentation, DiffView, DiffViewEvent};
pub use image_viewer::{FitMode, ImageFrame, ImageSize, ImageState, ImageViewer, ImageViewerEvent};
pub use log_stream::{LogEntry, LogStream, LogStreamState};
pub use markdown::{
    Block, CellAlign, CodeBlock, CodeSpan, Document, ImageRequest, Inline, ListEntry, Markdown,
    MarkdownEvent,
};
pub use message_list::{
    Attachment, DeliveryState, Message, MessageBody, MessageList, Reaction, streaming_since,
};
pub use transport::{
    BufferedRange, TrackStep, TransportBar, TransportDuration, TransportEvent, TransportState,
};
