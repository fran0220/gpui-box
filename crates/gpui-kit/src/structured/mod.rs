//! Structured data a host hands over: a value tree, and a form generated from
//! a description of one.
//!
//! Neither surface takes a serialization dependency. [`JsonValue`] and
//! [`Schema`] are the smallest shapes that express what has to be drawn, and a
//! host converts into them from whatever crate it already parses with. That
//! keeps this crate product-neutral in the way `SplitLayout` already is: the
//! records are plain, and choosing a format stays the host's decision.
//!
//! Both refuse to be quiet about what they cannot show. A value that is
//! withheld reads as withheld rather than as absent, and a schema field the
//! form cannot draw is reported in place of the control rather than dropped.

pub mod json_view;
pub mod schema_form;

pub use json_view::{JsonValue, JsonView, ValueKind};
pub use schema_form::{
    DefaultSchemaFilePolicy, FieldValue, FieldVisibility, HiddenSubmission, NumberBounds, Schema,
    SchemaChoice, SchemaField, SchemaFilePolicy, SchemaFileRequest, SchemaForm, SchemaFormEvent,
    SchemaKind, SharedSchemaFilePolicy, UnrenderableField, installed_schema_file_policy,
    reset_schema_file_policy, set_schema_file_policy,
};
