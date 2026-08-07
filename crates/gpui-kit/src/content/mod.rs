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

pub mod markdown;
pub mod message_list;

pub use markdown::{
    Block, CellAlign, CodeBlock, CodeSpan, Document, ImageRequest, Inline, ListEntry, Markdown,
    MarkdownEvent,
};
pub use message_list::{
    Attachment, DeliveryState, Message, MessageBody, MessageList, Reaction, streaming_since,
};
