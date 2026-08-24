//! A form generated from a description of the arguments a call takes.
//!
//! # Why there is a schema type here
//!
//! For the same reason [`JsonValue`](super::JsonValue) exists: this crate
//! takes no serialization dependency, so it cannot read a schema document. A
//! host converts whatever schema dialect it already parses into [`Schema`],
//! which describes only what a form has to draw — a name, a label, whether the
//! value is required, and which control the value is edited with.
//!
//! # A field the form cannot draw says so
//!
//! This is the rule the whole component is built around. A host converting a
//! schema it does not fully understand puts
//! [`SchemaKind::Unrenderable`] in place of the field, with the reason in its
//! own words; the form also refuses a few shapes on its own, such as a choice
//! among no choices. Either way the field keeps its place, keeps its label,
//! and states that it cannot be filled in here — and
//! [`SchemaForm::values`] still reports it, as
//! [`FieldValue::Unrenderable`], so a caller cannot collect the answers and
//! not notice one is missing.
//!
//! A form that quietly dropped a required argument it did not understand would
//! produce an invalid call, and the reader would be told they got it wrong.
//!
//! # Whose validation is on screen
//!
//! The form's required/range checks, caller-owned field validation, and
//! caller-owned whole-form validation stay separate. [`SchemaForm::validate`]
//! marks only what this component can judge; [`SchemaForm::set_field_validation`]
//! and [`SchemaForm::set_validation`] advance checks owned by the caller. A
//! whole-form refusal never paints every field red.
//!
//! # Visibility is caller-owned
//!
//! Conditional rules stay with the host. [`SchemaForm::set_field_visibility`]
//! only records their result: whether a field is visible and, when hidden,
//! whether its held value belongs in [`SchemaForm::submission_values`]. Hidden
//! fields are never field-validated because a person cannot repair a control
//! they cannot reach. [`SchemaForm::values`] remains the complete held-value
//! inventory, independent of presentation and submission policy.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext, Context, Entity, EventEmitter, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, Styled, Subscription, Window, div, prelude::FluentBuilder,
    px,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, Surface, TextTone, TypeScale};

use crate::controls::button::{Button, IconButton};
use crate::controls::combobox::{Combobox, ComboboxEvent};
use crate::controls::dropzone::Dropzone;
use crate::controls::form_field::FormField;
use crate::controls::input::{TextInput, TextInputEvent};
use crate::controls::number_input::{NumberInput, NumberInputEvent};
use crate::controls::select::{Select, SelectEvent, SelectOption};
use crate::controls::tag_input::{TagInput, TagInputEvent};
use crate::controls::toggle::Switch;
use crate::datetime::{
    DateInput, DateInputEvent, Day, RangePicker, RangePickerEvent, TimeInput, TimeInputEvent,
    installed_adapter,
};
use crate::display::badge::Tone;
use crate::display::status::Callout;
use crate::foundation::direction::{ActiveDirection, DirectionalExt};
use crate::foundation::{Disableable, Ident, Sizable, StyledExt, rule, text};
use crate::state::ValidationState;
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

/// The file field the form is asking the host to acquire paths for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaFileRequest {
    pub path: SharedString,
    pub label: SharedString,
    pub max: Option<usize>,
}

/// Host policy for schema file fields.
///
/// The form owns the drop target, selected-path rows, and schema maximum. The
/// host owns which paths may become answers and how one is named on screen. A
/// browse action emits [`SchemaFormEvent::FilesRequested`]; the host opens its
/// picker and returns paths with [`SchemaForm::set_files`].
pub trait SchemaFilePolicy {
    /// Accepts or refuses one complete candidate selection. A refusal is
    /// shown verbatim and changes none of the paths the form already holds.
    fn accept(&self, _request: &SchemaFileRequest, _paths: &[PathBuf]) -> Result<(), SharedString> {
        Ok(())
    }

    /// Names a selected path on screen. The name is deliberately omitted from
    /// semantic snapshots because a pathname is user-generated content.
    fn display_name(&self, path: &Path) -> SharedString;
}

pub type SharedSchemaFilePolicy = Rc<dyn SchemaFilePolicy>;

/// Opt-in policy for ordinary files with no host-specific restrictions.
///
/// Installing this is an explicit statement that any file path the platform
/// returns is acceptable. Applications with workspace, extension, or access
/// policy install their own implementation instead.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultSchemaFilePolicy;

impl SchemaFilePolicy for DefaultSchemaFilePolicy {
    fn display_name(&self, path: &Path) -> SharedString {
        path.file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned()
            .into()
    }
}

struct InstalledSchemaFiles(SharedSchemaFilePolicy);

impl gpui::Global for InstalledSchemaFiles {}

/// Reads the file policy a host installed, if any.
pub fn installed_schema_file_policy(cx: &App) -> Option<SharedSchemaFilePolicy> {
    cx.try_global::<InstalledSchemaFiles>()
        .map(|installed| Rc::clone(&installed.0))
}

/// Installs the policy schema file fields use. Replaces any previous policy.
pub fn set_schema_file_policy(policy: impl SchemaFilePolicy + 'static, cx: &mut App) {
    cx.set_global(InstalledSchemaFiles(Rc::new(policy)));
    cx.refresh_windows();
}

/// Removes the installed policy so schema file fields remain visibly
/// unrenderable rather than silently accepting paths.
pub fn reset_schema_file_policy(cx: &mut App) {
    if cx.has_global::<InstalledSchemaFiles>() {
        cx.remove_global::<InstalledSchemaFiles>();
        cx.refresh_windows();
    }
}

/// One option a closed or open choice offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaChoice {
    id: SharedString,
    label: SharedString,
    description: Option<SharedString>,
}

impl SchemaChoice {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: None,
        }
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    fn option(&self) -> SelectOption {
        let option = SelectOption::new(self.id.clone(), self.label.clone());
        match &self.description {
            Some(description) => option.description(description.clone()),
            None => option,
        }
    }
}

/// What a number may be, as far as the schema said.
///
/// Every bound is optional because a schema is allowed to state none of them,
/// and a form that invented a range would refuse values the host accepts.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NumberBounds {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
}

impl NumberBounds {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn min(mut self, min: f64) -> Self {
        self.min = Some(min);
        self
    }

    pub fn max(mut self, max: f64) -> Self {
        self.max = Some(max);
        self
    }

    pub fn step(mut self, step: f64) -> Self {
        self.step = Some(step);
        self
    }
}

/// What a value is, and therefore which control edits it.
#[derive(Debug, Clone, PartialEq)]
pub enum SchemaKind {
    Text {
        placeholder: Option<SharedString>,
        /// Drawn as dots, and kept out of every snapshot.
        secret: bool,
    },
    Number(NumberBounds),
    /// A number with no fractional part, which is a different control setting
    /// rather than a different control.
    Integer(NumberBounds),
    Boolean,
    /// One of these, and nothing else.
    Enum(Vec<SchemaChoice>),
    /// One of these, or something the reader types.
    OpenEnum(Vec<SchemaChoice>),
    /// A list of short values.
    TextList {
        max: Option<usize>,
    },
    /// Fields under a name of their own.
    Object(Vec<SchemaField>),
    /// A calendar day, edited with [`crate::datetime::DateInput`] when the
    /// host has installed a date adapter. Without one the field stays
    /// unrenderable rather than inventing a calendar.
    Date,
    /// A time of day, edited with [`crate::datetime::TimeInput`] when a date
    /// adapter is present.
    Time,
    /// Two days, edited with [`crate::datetime::RangePicker`] when a date
    /// adapter is present.
    DateRange,
    /// Paths the host already holds, collected through [`Dropzone`].
    Files {
        max: Option<usize>,
    },
    /// Repeating child objects. Each item is addressed as `parent[i].child`.
    List {
        item: Box<SchemaField>,
        max: Option<usize>,
    },
    /// The host could not express this field, and said so rather than
    /// dropping it. The text is the host's reason and is shown verbatim.
    Unrenderable(SharedString),
}

/// One argument.
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaField {
    name: SharedString,
    label: Option<SharedString>,
    description: Option<SharedString>,
    required: bool,
    kind: SchemaKind,
}

impl SchemaField {
    pub fn new(name: impl Into<SharedString>, kind: SchemaKind) -> Self {
        Self {
            name: name.into(),
            label: None,
            description: None,
            required: false,
            kind,
        }
    }

    /// What the reader sees. Without one the field's own name is shown, which
    /// is what a schema that named nothing else leaves to work with.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub fn name(&self) -> &SharedString {
        &self.name
    }

    pub fn kind(&self) -> &SchemaKind {
        &self.kind
    }

    pub fn is_required(&self) -> bool {
        self.required
    }

    fn shown_label(&self) -> SharedString {
        self.label.clone().unwrap_or_else(|| self.name.clone())
    }
}

/// The arguments a call takes, in the order they should be filled in.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Schema {
    fields: Vec<SchemaField>,
}

impl Schema {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn field(mut self, field: SchemaField) -> Self {
        self.fields.push(field);
        self
    }

    pub fn fields(mut self, fields: impl IntoIterator<Item = SchemaField>) -> Self {
        self.fields.extend(fields);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

/// A field that has to be filled in somewhere other than this form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnrenderableField {
    /// The path the field would have had in the answer, slash-joined through
    /// any objects above it.
    pub path: SharedString,
    pub label: SharedString,
    pub required: bool,
    /// Why, in the words of whoever refused: the host's, or this library's
    /// when the form itself is the one refusing.
    pub reason: SharedString,
}

/// What one field currently holds.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    Text(SharedString),
    Number(f64),
    Boolean(bool),
    Choice(SharedString),
    List(Vec<SharedString>),
    /// Paths the installed host policy accepted. Paths are returned to the
    /// caller but never copied into semantic snapshots.
    Files(Vec<PathBuf>),
    /// How many repeated sections the list currently holds. Their individual
    /// answers follow under paths such as `parent[0].child`.
    ItemCount(usize),
    /// A calendar day the host adapter already accepted.
    Day(i64),
    /// A time of day, as hour / minute / optional second.
    Time {
        hour: u32,
        minute: u32,
        second: Option<u32>,
    },
    /// Two days, start then optional end.
    Range {
        start: i64,
        end: Option<i64>,
    },
    /// Nothing was entered. Distinct from an empty string, which somebody
    /// typed on purpose.
    Absent,
    /// The form could not draw this field, so it holds nothing and never
    /// could. Reported rather than omitted.
    Unrenderable,
}

/// Whether a hidden field's held value belongs in the caller's submission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HiddenSubmission {
    /// Leave the field and its subtree out of [`SchemaForm::submission_values`].
    Omit,
    /// Keep the field and its subtree in [`SchemaForm::submission_values`].
    Include,
}

/// Whether one schema field participates in this form presentation.
///
/// Hidden fields are not rendered or field-validated. A hidden object or
/// repeating list applies its submission policy to the complete subtree, so a
/// descendant cannot accidentally reappear or use a contradictory policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FieldVisibility {
    #[default]
    Visible,
    Hidden {
        submission: HiddenSubmission,
    },
}

impl FieldVisibility {
    fn is_visible(self) -> bool {
        matches!(self, Self::Visible)
    }
}

/// What the form reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaFormEvent {
    /// The field at this path changed. The value is read from
    /// [`SchemaForm::values`], because a form has more than one of them.
    Changed(SharedString),
    /// The person asked to browse for this file field. The form deliberately
    /// opens no OS picker; the host acquires paths and returns them through
    /// [`SchemaForm::set_files`].
    FilesRequested(SchemaFileRequest),
    /// The primary key was pressed in a field. The form submits nothing.
    Submitted,
}

impl EventEmitter<SchemaFormEvent> for SchemaForm {}

struct SelectedFile {
    id: u64,
    path: PathBuf,
    label: SharedString,
}

struct FilesControl {
    request: SchemaFileRequest,
    policy: SharedSchemaFilePolicy,
    selected: Vec<SelectedFile>,
    next_id: u64,
    refusal: Option<SharedString>,
}

struct RepeatedItem {
    id: u64,
    form: Entity<SchemaForm>,
    _subscription: Subscription,
}

struct RepeatedControl {
    item: SchemaField,
    max: Option<usize>,
    items: Vec<RepeatedItem>,
    next_id: u64,
}

/// The control that edits one field, and where the value lives.
enum Control {
    Text(Entity<TextInput>),
    Number(Entity<NumberInput>),
    /// A switch is a builder rather than a view, so the draft is here.
    Boolean(bool),
    Choice(Entity<Select>),
    OpenChoice(Entity<Combobox>),
    List(Entity<TagInput>),
    Date(Entity<DateInput>),
    Time(Entity<TimeInput>),
    DateRange(Entity<RangePicker>),
    Files(FilesControl),
    Repeated(RepeatedControl),
    /// A heading over the fields beneath it. It holds nothing.
    Group,
    Unrenderable(SharedString),
}

/// One field, flattened out of however many objects it sat inside.
struct Field {
    path: SharedString,
    label: SharedString,
    description: Option<SharedString>,
    required: bool,
    level: u32,
    control: Control,
}

/// A form built from a schema.
///
/// It is a view rather than a builder because the fields it composes —
/// [`TextInput`], [`NumberInput`], [`Select`], [`Combobox`], [`TagInput`] —
/// each own a caret, an open menu, or a selection that has to survive a frame.
pub struct SchemaForm {
    ident: Ident,
    /// Positional answer prefix when this form is one stable repeated item.
    /// Reordering changes the prefix without changing any control identity.
    export_prefix: SharedString,
    fields: Vec<Field>,
    /// Validation the host owns, by field path. A missing entry means this
    /// field has no host-side validation contract; explicit Pending does.
    field_validation: BTreeMap<SharedString, ValidationState>,
    /// Validation about the submission as a whole, kept apart from fields.
    validation: Option<ValidationState>,
    /// Caller-owned conditional presentation, by local field path. Missing
    /// entries are visible; hidden object fields govern their whole subtree.
    field_visibility: BTreeMap<SharedString, FieldVisibility>,
    /// What the form worked out is missing, by path. Cleared by every edit to
    /// that field, so an answered complaint does not stay on screen.
    derived_errors: BTreeMap<SharedString, SharedString>,
    unrenderable: Vec<UnrenderableField>,
    base_unrenderable: usize,
    size: ControlSize,
    disabled: bool,
    _subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for SchemaForm {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SchemaForm")
            .field("ident", &self.ident)
            .field("fields", &self.fields.len())
            .field("unrenderable", &self.unrenderable.len())
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl SchemaForm {
    pub fn new(
        ident: impl Into<Ident>,
        schema: Schema,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let ident = ident.into();
        let mut form = Self {
            ident,
            export_prefix: SharedString::default(),
            fields: Vec::new(),
            field_validation: BTreeMap::new(),
            validation: None,
            field_visibility: BTreeMap::new(),
            derived_errors: BTreeMap::new(),
            unrenderable: Vec::new(),
            base_unrenderable: 0,
            size: ControlSize::Md,
            disabled: false,
            _subscriptions: Vec::new(),
        };
        let ident = form.ident.clone();
        form.build(&schema.fields, "", 1, &ident, window, cx);
        form.base_unrenderable = form.unrenderable.len();
        form
    }

    fn build(
        &mut self,
        fields: &[SchemaField],
        prefix: &str,
        level: u32,
        ident: &Ident,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for field in fields {
            let path = if prefix.is_empty() {
                field.name.clone()
            } else {
                SharedString::from(format!("{prefix}/{}", field.name))
            };
            let field_ident = ident.child(path.as_ref());
            let label = field.shown_label();
            let control = self.control_for(field, &path, &field_ident, window, cx);

            if let Control::Unrenderable(reason) = &control {
                self.unrenderable.push(UnrenderableField {
                    path: path.clone(),
                    label: label.clone(),
                    required: field.required,
                    reason: reason.clone(),
                });
            }

            let nested = match &field.kind {
                SchemaKind::Object(children) => Some(children.clone()),
                _ => None,
            };

            self.fields.push(Field {
                path: path.clone(),
                label,
                description: field.description.clone(),
                required: field.required,
                level,
                control,
            });

            if let Some(children) = nested {
                self.build(&children, path.as_ref(), level + 1, ident, window, cx);
            }
        }
    }

    fn control_for(
        &mut self,
        field: &SchemaField,
        path: &SharedString,
        ident: &Ident,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Control {
        let control_ident = ident.child("control");
        match &field.kind {
            SchemaKind::Unrenderable(reason) => Control::Unrenderable(reason.clone()),
            // A choice among nothing is not a control the form can draw, and
            // drawing an empty menu would look like a list that had not
            // loaded. The form refuses this one itself.
            SchemaKind::Enum(choices) | SchemaKind::OpenEnum(choices) if choices.is_empty() => {
                Control::Unrenderable(cx.strings().text(StringKey::SchemaNoChoices))
            }
            SchemaKind::Object(_) => Control::Group,
            SchemaKind::Text {
                placeholder,
                secret,
            } => {
                let secret = *secret;
                let placeholder = placeholder.clone();
                let input = cx.new(|cx| {
                    let mut input = TextInput::new(control_ident, window, cx)
                        .secret(secret)
                        .required(field.required);
                    if let Some(placeholder) = placeholder {
                        input = input.placeholder(placeholder);
                    }
                    input
                });
                self.watch_text(path, &input, cx);
                Control::Text(input)
            }
            SchemaKind::Number(bounds) | SchemaKind::Integer(bounds) => {
                let integer = matches!(field.kind, SchemaKind::Integer(_));
                let bounds = *bounds;
                let required = field.required;
                let label = field.shown_label();
                let number = cx.new(|cx| {
                    let mut number = NumberInput::new(control_ident, window, cx)
                        .required(required)
                        .name(label);
                    if let Some(min) = bounds.min {
                        number = number.min(min);
                    }
                    if let Some(max) = bounds.max {
                        number = number.max(max);
                    }
                    number = number.step(bounds.step.unwrap_or(1.0));
                    if integer {
                        number = number.precision(0);
                    }
                    number
                });
                self.watch_number(path, &number, cx);
                Control::Number(number)
            }
            SchemaKind::Boolean => Control::Boolean(false),
            SchemaKind::Enum(choices) => {
                let options: Vec<SelectOption> = choices.iter().map(SchemaChoice::option).collect();
                let name = field.shown_label();
                let select = cx.new(|cx| {
                    Select::new(control_ident, window, cx)
                        .name(name)
                        .options(options)
                });
                self.watch_select(path, &select, cx);
                Control::Choice(select)
            }
            SchemaKind::OpenEnum(choices) => {
                let options: Vec<SelectOption> = choices.iter().map(SchemaChoice::option).collect();
                let name = field.shown_label();
                let combobox = cx.new(|cx| {
                    Combobox::new(control_ident, window, cx)
                        .name(name)
                        .options(options)
                        .allow_custom(true)
                });
                self.watch_combobox(path, &combobox, cx);
                Control::OpenChoice(combobox)
            }
            SchemaKind::TextList { max } => {
                let max = *max;
                let tags = cx.new(|cx| {
                    let field = TagInput::new(control_ident, window, cx);
                    match max {
                        Some(max) => field.max(max),
                        None => field,
                    }
                });
                self.watch_tags(path, &tags, cx);
                Control::List(tags)
            }
            // These shapes are named so a host can describe a settings page
            // without dropping fields. Wiring them to DateInput / Dropzone
            // still needs a host adapter or a file policy; until that is
            // supplied the field stays visible as unrenderable.
            SchemaKind::Date => match installed_adapter(cx) {
                Some(adapter) => {
                    let input = cx.new(|cx| DateInput::new(control_ident, adapter, window, cx));
                    self.watch_date(path, &input, cx);
                    Control::Date(input)
                }
                None => Control::Unrenderable(cx.strings().text(StringKey::SchemaNeedsAdapter)),
            },
            SchemaKind::Time => match installed_adapter(cx) {
                Some(adapter) => {
                    let input = cx.new(|cx| TimeInput::new(control_ident, adapter, window, cx));
                    self.watch_time(path, &input, cx);
                    Control::Time(input)
                }
                None => Control::Unrenderable(cx.strings().text(StringKey::SchemaNeedsAdapter)),
            },
            SchemaKind::DateRange => match installed_adapter(cx) {
                Some(adapter) => {
                    let picker = cx.new(|cx| RangePicker::new(control_ident, adapter, window, cx));
                    self.watch_range(path, &picker, cx);
                    Control::DateRange(picker)
                }
                None => Control::Unrenderable(cx.strings().text(StringKey::SchemaNeedsAdapter)),
            },
            SchemaKind::Files { max } => match installed_schema_file_policy(cx) {
                Some(policy) => Control::Files(FilesControl {
                    request: SchemaFileRequest {
                        path: path.clone(),
                        label: field.shown_label(),
                        max: *max,
                    },
                    policy,
                    selected: Vec::new(),
                    next_id: 0,
                    refusal: None,
                }),
                None => Control::Unrenderable(cx.strings().text(StringKey::SchemaNeedsHost)),
            },
            SchemaKind::List { item, max } => Control::Repeated(RepeatedControl {
                item: item.as_ref().clone(),
                max: *max,
                items: Vec::new(),
                next_id: 0,
            }),
        }
    }

    fn watch_text(
        &mut self,
        path: &SharedString,
        input: &Entity<TextInput>,
        cx: &mut Context<Self>,
    ) {
        let path = path.clone();
        self._subscriptions.push(cx.subscribe(
            input,
            move |form, _, event: &TextInputEvent, cx| match event {
                TextInputEvent::Change(_) => form.changed(path.clone(), cx),
                TextInputEvent::Submit => cx.emit(SchemaFormEvent::Submitted),
                _ => {}
            },
        ));
    }

    fn watch_number(
        &mut self,
        path: &SharedString,
        number: &Entity<NumberInput>,
        cx: &mut Context<Self>,
    ) {
        let path = path.clone();
        self._subscriptions.push(cx.subscribe(
            number,
            move |form, number, event: &NumberInputEvent, cx| match event {
                NumberInputEvent::Changed(value) => {
                    let value = *value;
                    number.update(cx, |number, cx| number.set_value(value, cx));
                    form.changed(path.clone(), cx);
                }
                NumberInputEvent::Unparsable(_) => form.changed(path.clone(), cx),
                NumberInputEvent::Submit => cx.emit(SchemaFormEvent::Submitted),
            },
        ));
    }

    fn watch_select(
        &mut self,
        path: &SharedString,
        select: &Entity<Select>,
        cx: &mut Context<Self>,
    ) {
        let path = path.clone();
        self._subscriptions.push(cx.subscribe(
            select,
            move |form, select, event: &SelectEvent, cx| {
                if let SelectEvent::Selected(id) = event {
                    let id = id.clone();
                    select.update(cx, |select, cx| select.set_selected(Some(id), cx));
                    form.changed(path.clone(), cx);
                }
            },
        ));
    }

    fn watch_combobox(
        &mut self,
        path: &SharedString,
        combobox: &Entity<Combobox>,
        cx: &mut Context<Self>,
    ) {
        let path = path.clone();
        self._subscriptions.push(cx.subscribe(
            combobox,
            move |form, combobox, event: &ComboboxEvent, cx| match event {
                ComboboxEvent::Selected(id) => {
                    let id = id.clone();
                    combobox.update(cx, |combobox, cx| combobox.set_selected(Some(id), cx));
                    form.changed(path.clone(), cx);
                }
                ComboboxEvent::Custom(_) => form.changed(path.clone(), cx),
                _ => {}
            },
        ));
    }

    fn watch_tags(&mut self, path: &SharedString, tags: &Entity<TagInput>, cx: &mut Context<Self>) {
        let path = path.clone();
        self._subscriptions.push(cx.subscribe(
            tags,
            move |form, tags, event: &TagInputEvent, cx| {
                let next = match event {
                    TagInputEvent::Added(value) => {
                        let mut next = tags.read(cx).current().to_vec();
                        next.push(value.clone());
                        Some(next)
                    }
                    TagInputEvent::Removed(value) => Some(
                        tags.read(cx)
                            .current()
                            .iter()
                            .filter(|tag| *tag != value)
                            .cloned()
                            .collect(),
                    ),
                    // A duplicate and a full field are refusals the tag field
                    // already shows where the typist is looking. Applying one
                    // would be applying a change nobody accepted.
                    _ => None,
                };
                if let Some(next) = next {
                    tags.update(cx, |tags, cx| tags.set_tags(next, cx));
                    form.changed(path.clone(), cx);
                }
            },
        ));
    }

    fn watch_date(
        &mut self,
        path: &SharedString,
        input: &Entity<DateInput>,
        cx: &mut Context<Self>,
    ) {
        let path = path.clone();
        self._subscriptions.push(cx.subscribe(
            input,
            move |form, _, event: &DateInputEvent, cx| match event {
                DateInputEvent::Changed(_) | DateInputEvent::Unparsable { .. } => {
                    form.changed(path.clone(), cx)
                }
                DateInputEvent::Submit => cx.emit(SchemaFormEvent::Submitted),
                _ => {}
            },
        ));
    }

    fn watch_time(
        &mut self,
        path: &SharedString,
        input: &Entity<TimeInput>,
        cx: &mut Context<Self>,
    ) {
        let path = path.clone();
        self._subscriptions.push(cx.subscribe(
            input,
            move |form, _, event: &TimeInputEvent, cx| match event {
                TimeInputEvent::Changed(_) => form.changed(path.clone(), cx),
            },
        ));
    }

    fn watch_range(
        &mut self,
        path: &SharedString,
        picker: &Entity<RangePicker>,
        cx: &mut Context<Self>,
    ) {
        let path = path.clone();
        self._subscriptions.push(cx.subscribe(
            picker,
            move |form, _, event: &RangePickerEvent, cx| match event {
                RangePickerEvent::StartPicked(_) | RangePickerEvent::EndPicked(_) => {
                    form.changed(path.clone(), cx)
                }
            },
        ));
    }

    fn changed(&mut self, path: SharedString, cx: &mut Context<Self>) {
        // The form's own complaint was about this field being empty, and it is
        // not empty any more; the host's stands until the host withdraws it.
        self.derived_errors.remove(&path);
        self.sync_control_validity(cx);
        cx.emit(SchemaFormEvent::Changed(path));
        cx.notify();
    }

    fn apply_files(
        &mut self,
        path: &SharedString,
        paths: Vec<PathBuf>,
        append: bool,
        cx: &mut Context<Self>,
    ) -> Result<bool, SharedString> {
        let Some(field) = self.fields.iter().find(|field| field.path == *path) else {
            return Ok(false);
        };
        let Control::Files(files) = &field.control else {
            return Ok(false);
        };

        let mut candidates = if append {
            files
                .selected
                .iter()
                .map(|file| file.path.clone())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        for candidate in paths {
            if !candidates.contains(&candidate) {
                candidates.push(candidate);
            }
        }

        if files
            .request
            .max
            .is_some_and(|maximum| candidates.len() > maximum)
        {
            let maximum = files.request.max.expect("checked above");
            let refusal = cx.strings().format_plural(
                StringKey::SchemaFileMaximumOne,
                StringKey::SchemaFilesMaximum,
                cx.numbers().plural(maximum),
                &[cx.numbers().count(maximum).as_ref()],
            );
            if let Some(field) = self.fields.iter_mut().find(|field| field.path == *path)
                && let Control::Files(files) = &mut field.control
            {
                files.refusal = Some(refusal.clone());
            }
            cx.notify();
            return Err(refusal);
        }

        if let Err(refusal) = files.policy.accept(&files.request, &candidates) {
            if let Some(field) = self.fields.iter_mut().find(|field| field.path == *path)
                && let Control::Files(files) = &mut field.control
            {
                files.refusal = Some(refusal.clone());
            }
            cx.notify();
            return Err(refusal);
        }

        let policy = Rc::clone(&files.policy);
        let previous = files
            .selected
            .iter()
            .map(|file| (file.path.clone(), file.id, file.label.clone()))
            .collect::<Vec<_>>();
        let mut next_id = files.next_id;
        let selected = candidates
            .into_iter()
            .map(|candidate| {
                if let Some((_, id, label)) =
                    previous.iter().find(|(path, _, _)| path == &candidate)
                {
                    SelectedFile {
                        id: *id,
                        path: candidate,
                        label: label.clone(),
                    }
                } else {
                    let id = next_id;
                    next_id += 1;
                    let label = policy.display_name(&candidate);
                    SelectedFile {
                        id,
                        path: candidate,
                        label,
                    }
                }
            })
            .collect::<Vec<_>>();
        let changed = previous.len() != selected.len()
            || previous
                .iter()
                .zip(&selected)
                .any(|((path, _, _), selected)| path != &selected.path);

        if let Some(field) = self.fields.iter_mut().find(|field| field.path == *path)
            && let Control::Files(files) = &mut field.control
        {
            files.selected = selected;
            files.next_id = next_id;
            files.refusal = None;
        }
        if changed {
            self.changed(path.clone(), cx);
        } else {
            cx.notify();
        }
        Ok(changed)
    }

    /// Replaces the paths in one file field after applying its installed host
    /// policy and schema maximum. Returns `Ok(false)` when the path does not
    /// name a file field.
    pub fn set_files(
        &mut self,
        path: impl Into<SharedString>,
        files: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) -> Result<bool, SharedString> {
        let path = path.into();
        if let Some((form, child_path)) = self.repeated_child(path.as_ref()) {
            return form.update(cx, |form, cx| form.set_files(child_path, files, cx));
        }
        self.apply_files(&path, files, false, cx)
    }

    fn remove_file(&mut self, path: &SharedString, id: u64, cx: &mut Context<Self>) {
        let mut changed = false;
        if let Some(field) = self.fields.iter_mut().find(|field| field.path == *path)
            && let Control::Files(files) = &mut field.control
        {
            let before = files.selected.len();
            files.selected.retain(|file| file.id != id);
            files.refusal = None;
            changed = before != files.selected.len();
        }
        if changed {
            self.changed(path.clone(), cx);
        }
    }

    fn repeated_schema(item: &SchemaField) -> Schema {
        match &item.kind {
            SchemaKind::Object(children) => Schema::new().fields(children.clone()),
            _ => Schema::new().field(item.clone()),
        }
    }

    /// Adds one repeated section. The section keeps its semantic identity
    /// across later reorders; only its caller-facing `parent[i].child` paths
    /// change with its position.
    pub fn add_list_item(
        &mut self,
        path: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let path = path.into();
        if let Some((form, child_path)) = self.repeated_child(path.as_ref()) {
            return form.update(cx, |form, cx| form.add_list_item(child_path, window, cx));
        }
        let Some((item, id, index)) = self
            .fields
            .iter_mut()
            .find(|field| field.path == path)
            .and_then(|field| match &mut field.control {
                Control::Repeated(repeated)
                    if repeated
                        .max
                        .is_none_or(|maximum| repeated.items.len() < maximum) =>
                {
                    let id = repeated.next_id;
                    repeated.next_id += 1;
                    Some((repeated.item.clone(), id, repeated.items.len()))
                }
                _ => None,
            })
        else {
            return false;
        };

        let item_ident = self
            .ident
            .child(path.as_ref())
            .child("control")
            .child(format!("item-{id}"));
        let schema = Self::repeated_schema(&item);
        let form = cx.new(|cx| SchemaForm::new(item_ident, schema, window, cx));
        let export_prefix = Self::exported_path(
            self.export_prefix.as_ref(),
            format!("{path}[{index}]").as_ref(),
        );
        form.update(cx, |form, cx| form.set_export_prefix(export_prefix, cx));
        if self.disabled {
            form.update(cx, |form, cx| form.set_disabled(true, cx));
        }
        let parent_path = path.clone();
        let subscription =
            cx.subscribe(
                &form,
                move |parent, _, event: &SchemaFormEvent, cx| match event {
                    SchemaFormEvent::Changed(child) => {
                        parent.repeated_child_changed(&parent_path, id, child, cx)
                    }
                    SchemaFormEvent::FilesRequested(request) => {
                        cx.emit(SchemaFormEvent::FilesRequested(request.clone()))
                    }
                    SchemaFormEvent::Submitted => cx.emit(SchemaFormEvent::Submitted),
                },
            );
        if let Some(field) = self.fields.iter_mut().find(|field| field.path == path)
            && let Control::Repeated(repeated) = &mut field.control
        {
            repeated.items.push(RepeatedItem {
                id,
                form,
                _subscription: subscription,
            });
        }
        self.refresh_repeated_unrenderable(cx);
        self.changed(path, cx);
        true
    }

    /// Removes one repeated section by its current index.
    pub fn remove_list_item(
        &mut self,
        path: impl Into<SharedString>,
        index: usize,
        cx: &mut Context<Self>,
    ) -> bool {
        let path = path.into();
        if let Some((form, child_path)) = self.repeated_child(path.as_ref()) {
            return form.update(cx, |form, cx| form.remove_list_item(child_path, index, cx));
        }
        let removed = self
            .fields
            .iter_mut()
            .find(|field| field.path == path)
            .is_some_and(|field| match &mut field.control {
                Control::Repeated(repeated) if index < repeated.items.len() => {
                    repeated.items.remove(index);
                    true
                }
                _ => false,
            });
        if removed {
            self.sync_repeated_export_paths(cx);
            self.refresh_repeated_unrenderable(cx);
            self.changed(path, cx);
        }
        removed
    }

    /// Moves one repeated section to another current index.
    pub fn move_list_item(
        &mut self,
        path: impl Into<SharedString>,
        from: usize,
        to: usize,
        cx: &mut Context<Self>,
    ) -> bool {
        let path = path.into();
        if let Some((form, child_path)) = self.repeated_child(path.as_ref()) {
            return form.update(cx, |form, cx| form.move_list_item(child_path, from, to, cx));
        }
        let moved = self
            .fields
            .iter_mut()
            .find(|field| field.path == path)
            .is_some_and(|field| match &mut field.control {
                Control::Repeated(repeated)
                    if from < repeated.items.len() && to < repeated.items.len() && from != to =>
                {
                    let item = repeated.items.remove(from);
                    repeated.items.insert(to, item);
                    true
                }
                _ => false,
            });
        if moved {
            self.sync_repeated_export_paths(cx);
            self.refresh_repeated_unrenderable(cx);
            self.changed(path, cx);
        }
        moved
    }

    fn repeated_child_changed(
        &mut self,
        parent_path: &SharedString,
        id: u64,
        child_path: &SharedString,
        cx: &mut Context<Self>,
    ) {
        let index = self.repeated_index(parent_path, id);
        if let Some(index) = index {
            self.refresh_repeated_unrenderable(cx);
            self.changed(format!("{parent_path}[{index}].{child_path}").into(), cx);
        }
    }

    fn repeated_index(&self, parent_path: &SharedString, id: u64) -> Option<usize> {
        self.fields
            .iter()
            .find(|field| field.path == *parent_path)
            .and_then(|field| match &field.control {
                Control::Repeated(repeated) => repeated.items.iter().position(|item| item.id == id),
                _ => None,
            })
    }

    fn exported_path(prefix: &str, local: &str) -> SharedString {
        if prefix.is_empty() {
            local.to_owned().into()
        } else {
            format!("{prefix}.{local}").into()
        }
    }

    fn set_export_prefix(&mut self, prefix: SharedString, cx: &mut Context<Self>) {
        self.export_prefix = prefix;
        for field in &mut self.fields {
            if let Control::Files(files) = &mut field.control {
                files.request.path =
                    Self::exported_path(self.export_prefix.as_ref(), field.path.as_ref());
            }
        }
        self.sync_repeated_export_paths(cx);
    }

    fn sync_repeated_export_paths(&self, cx: &mut Context<Self>) {
        let children = self
            .fields
            .iter()
            .filter_map(|field| match &field.control {
                Control::Repeated(repeated) => Some(
                    repeated
                        .items
                        .iter()
                        .enumerate()
                        .map(|(index, item)| {
                            (
                                item.form.clone(),
                                Self::exported_path(
                                    self.export_prefix.as_ref(),
                                    format!("{}[{index}]", field.path).as_ref(),
                                ),
                            )
                        })
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            })
            .flatten()
            .collect::<Vec<_>>();
        for (form, prefix) in children {
            form.update(cx, |form, cx| form.set_export_prefix(prefix, cx));
        }
    }

    fn repeated_child(&self, path: &str) -> Option<(Entity<SchemaForm>, SharedString)> {
        self.fields.iter().find_map(|field| {
            let Control::Repeated(repeated) = &field.control else {
                return None;
            };
            let rest = path.strip_prefix(field.path.as_ref())?.strip_prefix('[')?;
            let (index, child_path) = rest.split_once("].")?;
            let index = index.parse::<usize>().ok()?;
            let item = repeated.items.get(index)?;
            Some((item.form.clone(), child_path.to_owned().into()))
        })
    }

    fn repeated_parent_visibility(&self, path: &str) -> Option<FieldVisibility> {
        self.fields.iter().find_map(|field| {
            let Control::Repeated(repeated) = &field.control else {
                return None;
            };
            let rest = path.strip_prefix(field.path.as_ref())?.strip_prefix('[')?;
            let (index, _) = rest.split_once("].")?;
            let index = index.parse::<usize>().ok()?;
            repeated.items.get(index)?;
            Some(self.visibility_for_local_path(field.path.as_ref()))
        })
    }

    /// Returns the effective local visibility, including a hidden object
    /// ancestor. The first hidden ancestor governs the whole subtree.
    fn visibility_for_local_path(&self, path: &str) -> FieldVisibility {
        let mut prefix = String::new();
        for segment in path.split('/') {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(segment);
            if let Some(visibility @ FieldVisibility::Hidden { .. }) =
                self.field_visibility.get(prefix.as_str()).copied()
            {
                return visibility;
            }
        }
        FieldVisibility::Visible
    }

    fn field_is_visible(&self, field: &Field) -> bool {
        self.visibility_for_local_path(field.path.as_ref())
            .is_visible()
    }

    fn refresh_repeated_unrenderable(&mut self, cx: &App) {
        self.unrenderable.truncate(self.base_unrenderable);
        for field in &self.fields {
            let Control::Repeated(repeated) = &field.control else {
                continue;
            };
            for (index, item) in repeated.items.iter().enumerate() {
                self.unrenderable
                    .extend(
                        item.form
                            .read(cx)
                            .unrenderable()
                            .iter()
                            .map(|unrenderable| UnrenderableField {
                                path: format!("{}[{index}].{}", field.path, unrenderable.path)
                                    .into(),
                                label: unrenderable.label.clone(),
                                required: unrenderable.required,
                                reason: unrenderable.reason.clone(),
                            }),
                    );
            }
        }
    }

    /// Advances caller-owned validation for one field.
    ///
    /// Pending and Validating block submission but never draw invalid chrome.
    /// A repeated field path such as `items[0].name` is routed to that stable
    /// child form.
    pub fn set_field_validation(
        &mut self,
        path: impl Into<SharedString>,
        validation: ValidationState,
        cx: &mut Context<Self>,
    ) -> bool {
        let path = path.into();
        if let Some((form, child_path)) = self.repeated_child(path.as_ref()) {
            return form.update(cx, |form, cx| {
                form.set_field_validation(child_path, validation, cx)
            });
        }
        if !self.fields.iter().any(|field| field.path == path) {
            return false;
        }
        self.field_validation.insert(path, validation);
        self.sync_control_validity(cx);
        cx.notify();
        true
    }

    /// The caller-owned validation registered for one current field path.
    ///
    /// Repeated item paths are resolved through their current position while
    /// the state itself stays with the stable child form across reorders.
    pub fn field_validation(&self, path: &str, cx: &App) -> Option<ValidationState> {
        if let Some((form, child_path)) = self.repeated_child(path) {
            return form.read(cx).field_validation(child_path.as_ref(), cx);
        }
        self.field_validation.get(path).cloned()
    }

    /// Removes caller-owned validation for one field. The form's own required
    /// and range checks remain independent.
    pub fn clear_field_validation(
        &mut self,
        path: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> bool {
        let path = path.into();
        if let Some((form, child_path)) = self.repeated_child(path.as_ref()) {
            return form.update(cx, |form, cx| form.clear_field_validation(child_path, cx));
        }
        if self.field_validation.remove(&path).is_none() {
            return false;
        }
        self.sync_control_validity(cx);
        cx.notify();
        true
    }

    /// Applies the caller's conditional-visibility result to one current
    /// field path. Returns `false` for a stale or unknown path.
    ///
    /// A hidden field is never field-validated: an invalid control nobody can
    /// reach must not become an invisible submission blocker. Use whole-form
    /// validation when a hidden caller-owned value is itself refused. Hiding
    /// an object or repeating list governs its complete subtree.
    pub fn set_field_visibility(
        &mut self,
        path: impl Into<SharedString>,
        visibility: FieldVisibility,
        cx: &mut Context<Self>,
    ) -> bool {
        let path = path.into();
        if let Some((form, child_path)) = self.repeated_child(path.as_ref()) {
            return form.update(cx, |form, cx| {
                form.set_field_visibility(child_path, visibility, cx)
            });
        }
        if !self.fields.iter().any(|field| field.path == path) {
            return false;
        }
        match visibility {
            FieldVisibility::Visible => {
                self.field_visibility.remove(&path);
            }
            FieldVisibility::Hidden { .. } => {
                self.field_visibility.insert(path, visibility);
            }
        }
        self.sync_control_validity(cx);
        cx.notify();
        true
    }

    /// Returns one current field path's effective visibility. A hidden object
    /// or repeating-list ancestor is returned for every descendant it governs.
    /// Repeated item paths resolve through the item's current position while
    /// their state stays with the stable child form across reorders.
    pub fn field_visibility(&self, path: &str, cx: &App) -> Option<FieldVisibility> {
        if let Some(parent_visibility @ FieldVisibility::Hidden { .. }) =
            self.repeated_parent_visibility(path)
        {
            return Some(parent_visibility);
        }
        if let Some((form, child_path)) = self.repeated_child(path) {
            return form.read(cx).field_visibility(child_path.as_ref(), cx);
        }
        self.fields
            .iter()
            .any(|field| field.path == path)
            .then(|| self.visibility_for_local_path(path))
    }

    /// Advances validation for the complete form. A form-level invalid reason
    /// is presented once and never copied onto individual fields.
    pub fn set_validation(&mut self, validation: ValidationState, cx: &mut Context<Self>) {
        self.validation = Some(validation);
        cx.notify();
    }

    /// The caller-owned validation registered for the complete form.
    pub fn validation(&self) -> Option<&ValidationState> {
        self.validation.as_ref()
    }

    /// Removes the whole-form validation contract.
    pub fn clear_validation(&mut self, cx: &mut Context<Self>) {
        self.validation = None;
        cx.notify();
    }

    /// Shows an error the host returned, next to the field it is about.
    ///
    /// This is the compatibility shorthand for setting that field to
    /// [`ValidationState::Invalid`].
    pub fn set_error(
        &mut self,
        path: impl Into<SharedString>,
        message: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        let _ = self.set_field_validation(path, ValidationState::invalid(message), cx);
    }

    /// Withdraws every error the host reported. What the form worked out for
    /// itself is untouched, because the host did not put it there.
    pub fn clear_host_errors(&mut self, cx: &mut Context<Self>) {
        self.field_validation
            .retain(|_, validation| !validation.is_invalid());
        if self
            .validation
            .as_ref()
            .is_some_and(ValidationState::is_invalid)
        {
            self.validation = None;
        }
        for item in self
            .fields
            .iter()
            .filter_map(|field| match &field.control {
                Control::Repeated(repeated) => Some(&repeated.items),
                _ => None,
            })
            .flatten()
        {
            item.form.update(cx, |form, cx| form.clear_host_errors(cx));
        }
        self.sync_control_validity(cx);
        cx.notify();
    }

    /// Marks every required field nobody filled in, and reports whether the
    /// form is answerable at all.
    ///
    /// A form holding a field it cannot draw is never answerable, whatever is
    /// typed into the rest of it. Neither is one holding a number outside the
    /// range the schema gave it: the control already draws that as wrong, and
    /// a form that called it answerable anyway would be contradicting what is
    /// on screen.
    pub fn validate(&mut self, cx: &mut Context<Self>) -> bool {
        self.derived_errors.clear();
        let missing: Vec<SharedString> = self
            .fields
            .iter()
            .filter(|field| {
                self.field_is_visible(field)
                    && field.required
                    && match self.value_of(field, cx) {
                        FieldValue::Absent => true,
                        FieldValue::Files(paths) => paths.is_empty(),
                        FieldValue::ItemCount(count) => count == 0,
                        _ => false,
                    }
            })
            .map(|field| field.path.clone())
            .collect();
        let message = cx.strings().text(StringKey::SchemaRequiredMissing);
        for path in missing {
            self.derived_errors.insert(path, message.clone());
        }
        let rejected: Vec<(SharedString, SharedString)> = self
            .fields
            .iter()
            .filter(|field| self.field_is_visible(field))
            .filter_map(|field| match &field.control {
                Control::Number(number) => number
                    .read(cx)
                    .invalid_reason(cx)
                    .map(|reason| (field.path.clone(), reason)),
                _ => None,
            })
            .collect();
        for (path, reason) in rejected {
            self.derived_errors.entry(path).or_insert(reason);
        }
        let mut repeated_answerable = true;
        for field in self
            .fields
            .iter()
            .filter(|field| self.field_is_visible(field))
        {
            if let Control::Repeated(repeated) = &field.control {
                for item in &repeated.items {
                    repeated_answerable &= item.form.update(cx, |form, cx| form.validate(cx));
                }
            }
        }
        self.refresh_repeated_unrenderable(cx);
        self.sync_control_validity(cx);
        cx.notify();
        repeated_answerable
            && self.derived_errors.is_empty()
            && !self.visible_unrenderable(cx).1
            && self.managed_validation_allows_submit(cx)
    }

    /// Every field and what it holds, including the ones the form could not
    /// draw. A caller that builds a call from this cannot lose a field without
    /// seeing it.
    pub fn values(&self, cx: &App) -> Vec<(SharedString, FieldValue)> {
        let mut values = Vec::new();
        for field in &self.fields {
            if matches!(field.control, Control::Group) {
                continue;
            }
            values.push((field.path.clone(), self.value_of(field, cx)));
            if let Control::Repeated(repeated) = &field.control {
                for (index, item) in repeated.items.iter().enumerate() {
                    values.extend(item.form.read(cx).values(cx).into_iter().map(
                        |(child_path, value)| {
                            (
                                format!("{}[{index}].{child_path}", field.path).into(),
                                value,
                            )
                        },
                    ));
                }
            }
        }
        values
    }

    /// The explicit submission set after applying caller-owned visibility.
    ///
    /// Unlike [`SchemaForm::values`], a hidden field with
    /// [`HiddenSubmission::Omit`] and its subtree are absent. A hidden field
    /// with [`HiddenSubmission::Include`] preserves its complete held subtree.
    pub fn submission_values(&self, cx: &App) -> Vec<(SharedString, FieldValue)> {
        let mut values = Vec::new();
        for field in &self.fields {
            let visibility = self.visibility_for_local_path(field.path.as_ref());
            if matches!(
                visibility,
                FieldVisibility::Hidden {
                    submission: HiddenSubmission::Omit
                }
            ) || matches!(field.control, Control::Group)
            {
                continue;
            }
            values.push((field.path.clone(), self.value_of(field, cx)));
            if let Control::Repeated(repeated) = &field.control {
                for (index, item) in repeated.items.iter().enumerate() {
                    let child_values = match visibility {
                        FieldVisibility::Visible => item.form.read(cx).submission_values(cx),
                        FieldVisibility::Hidden {
                            submission: HiddenSubmission::Include,
                        } => item.form.read(cx).values(cx),
                        FieldVisibility::Hidden {
                            submission: HiddenSubmission::Omit,
                        } => Vec::new(),
                    };
                    values.extend(child_values.into_iter().map(|(child_path, value)| {
                        (
                            format!("{}[{index}].{child_path}", field.path).into(),
                            value,
                        )
                    }));
                }
            }
        }
        values
    }

    /// The fields that have to be filled in somewhere else.
    pub fn unrenderable(&self) -> &[UnrenderableField] {
        &self.unrenderable
    }

    /// Whether any field the form could not draw is one the call requires.
    pub fn has_unrenderable_required(&self) -> bool {
        self.unrenderable.iter().any(|field| field.required)
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        for field in &self.fields {
            match &field.control {
                Control::Text(input) => {
                    input.update(cx, |input, cx| input.set_disabled(disabled, cx))
                }
                Control::Number(number) => {
                    number.update(cx, |number, cx| number.set_disabled(disabled, cx))
                }
                Control::Choice(select) => {
                    select.update(cx, |select, cx| select.set_disabled(disabled, cx))
                }
                Control::OpenChoice(combobox) => {
                    combobox.update(cx, |combobox, cx| combobox.set_disabled(disabled, cx))
                }
                Control::List(tags) => tags.update(cx, |tags, cx| tags.set_disabled(disabled, cx)),
                Control::Date(input) => {
                    input.update(cx, |input, cx| input.set_disabled(disabled, cx))
                }
                Control::Time(input) => {
                    input.update(cx, |input, cx| input.set_disabled(disabled, cx))
                }
                Control::DateRange(picker) => {
                    picker.update(cx, |picker, cx| picker.set_disabled(disabled, cx))
                }
                Control::Repeated(repeated) => {
                    for item in &repeated.items {
                        item.form
                            .update(cx, |form, cx| form.set_disabled(disabled, cx));
                    }
                }
                Control::Boolean(_)
                | Control::Files(_)
                | Control::Group
                | Control::Unrenderable(_) => {}
            }
        }
        cx.notify();
    }

    fn value_of(&self, field: &Field, cx: &App) -> FieldValue {
        match &field.control {
            Control::Text(input) => match input.read(cx).value() {
                text if text.is_empty() => FieldValue::Absent,
                text => FieldValue::Text(text.clone()),
            },
            Control::Number(number) => match number.read(cx).shown(cx) {
                Some(value) => FieldValue::Number(value),
                None => FieldValue::Absent,
            },
            Control::Boolean(on) => FieldValue::Boolean(*on),
            Control::Choice(select) => match select.read(cx).selected_id() {
                Some(id) => FieldValue::Choice(id.clone()),
                None => FieldValue::Absent,
            },
            Control::OpenChoice(combobox) => {
                let combobox = combobox.read(cx);
                match combobox.selected_id() {
                    Some(id) => FieldValue::Choice(id.clone()),
                    None => match combobox.query_text(cx) {
                        query if query.is_empty() => FieldValue::Absent,
                        query => FieldValue::Choice(query),
                    },
                }
            }
            Control::List(tags) => match tags.read(cx).current() {
                [] => FieldValue::Absent,
                tags => FieldValue::List(tags.to_vec()),
            },
            Control::Date(input) => match input.read(cx).parsed_day(cx) {
                Some(Day(day)) => FieldValue::Day(day),
                None => FieldValue::Absent,
            },
            Control::Time(input) => {
                let time = input.read(cx).current();
                FieldValue::Time {
                    hour: time.hour,
                    minute: time.minute,
                    second: time.second,
                }
            }
            Control::DateRange(picker) => match picker.read(cx).state() {
                crate::datetime::RangeState::Unset => FieldValue::Absent,
                crate::datetime::RangeState::Incomplete { start } => FieldValue::Range {
                    start: start.0,
                    end: None,
                },
                crate::datetime::RangeState::Complete { start, end }
                | crate::datetime::RangeState::Inverted { start, end } => FieldValue::Range {
                    start: start.0,
                    end: Some(end.0),
                },
            },
            Control::Files(files) => FieldValue::Files(
                files
                    .selected
                    .iter()
                    .map(|file| file.path.clone())
                    .collect(),
            ),
            Control::Repeated(repeated) => FieldValue::ItemCount(repeated.items.len()),
            Control::Unrenderable(_) => FieldValue::Unrenderable,
            Control::Group => FieldValue::Absent,
        }
    }

    /// The effective validation shown on a field. A host refusal outranks the
    /// form's own reason; local invalidity outranks non-failing host activity.
    ///
    /// A control that draws itself as invalid is asked why last of all, so the
    /// red border a number gets for leaving its range always arrives with the
    /// range beside it rather than on its own.
    fn validation_for(&self, field: &Field, cx: &App) -> ValidationState {
        if let Some(validation) = self
            .field_validation
            .get(&field.path)
            .filter(|validation| validation.is_invalid())
        {
            return validation.clone();
        }
        if let Some(error) = self.derived_errors.get(&field.path) {
            return ValidationState::invalid(error.clone());
        }
        if let Control::Number(number) = &field.control
            && let Some(reason) = number.read(cx).invalid_reason(cx)
        {
            return ValidationState::invalid(reason);
        }
        self.field_validation
            .get(&field.path)
            .cloned()
            .unwrap_or_default()
    }

    fn managed_validation_allows_submit(&self, cx: &App) -> bool {
        self.validation
            .as_ref()
            .is_none_or(|validation| matches!(validation, ValidationState::Valid))
            && self.field_validation.iter().all(|(path, validation)| {
                !self
                    .field_visibility(path, cx)
                    .is_some_and(FieldVisibility::is_visible)
                    || matches!(validation, ValidationState::Valid)
            })
    }

    fn validation_is_busy(&self, cx: &App) -> bool {
        self.validation
            .as_ref()
            .is_some_and(ValidationState::is_busy)
            || self
                .fields
                .iter()
                .filter(|field| self.field_is_visible(field))
                .any(|field| self.validation_for(field, cx).is_busy())
            || self
                .fields
                .iter()
                .filter(|field| self.field_is_visible(field))
                .any(|field| match &field.control {
                    Control::Repeated(repeated) => repeated
                        .items
                        .iter()
                        .any(|item| item.form.read(cx).validation_is_busy(cx)),
                    _ => false,
                })
    }

    fn validation_is_invalid(&self, cx: &App) -> bool {
        self.validation
            .as_ref()
            .is_some_and(ValidationState::is_invalid)
            || self
                .fields
                .iter()
                .filter(|field| self.field_is_visible(field))
                .any(|field| self.validation_for(field, cx).is_invalid())
            || self.visible_unrenderable(cx).1
            || self
                .fields
                .iter()
                .filter(|field| self.field_is_visible(field))
                .any(|field| match &field.control {
                    Control::Repeated(repeated) => repeated
                        .items
                        .iter()
                        .any(|item| item.form.read(cx).validation_is_invalid(cx)),
                    _ => false,
                })
    }

    /// Number of unrenderable fields currently presented, and whether one is
    /// required. The public inventory remains complete; this projection keeps
    /// hidden fields out of validation and the visible summary.
    fn visible_unrenderable(&self, cx: &App) -> (usize, bool) {
        let mut count = 0;
        let mut required = false;
        for field in self
            .fields
            .iter()
            .filter(|field| self.field_is_visible(field))
        {
            match &field.control {
                Control::Unrenderable(_) => {
                    count += 1;
                    required |= field.required;
                }
                Control::Repeated(repeated) => {
                    for item in &repeated.items {
                        let child = item.form.read(cx).visible_unrenderable(cx);
                        count += child.0;
                        required |= child.1;
                    }
                }
                _ => {}
            }
        }
        (count, required)
    }

    /// Tells every control whether the form is showing an error about it.
    ///
    /// The message and the control it is about have to agree. A red sentence
    /// under a field wearing its ordinary border says the field is fine and
    /// something else is wrong, which is the opposite of what happened. A
    /// number works this out for itself, so it is left alone.
    fn sync_control_validity(&mut self, cx: &mut Context<Self>) {
        let refused: Vec<(usize, bool)> = self
            .fields
            .iter()
            .enumerate()
            .map(|(index, field)| (index, self.validation_for(field, cx).is_invalid()))
            .collect();
        for (index, refused) in refused {
            match &self.fields[index].control {
                Control::Text(input) => {
                    input.update(cx, |input, cx| input.set_invalid(refused, cx))
                }
                Control::Number(number) => {
                    number.update(cx, |number, cx| number.set_invalid(refused, cx))
                }
                Control::Choice(select) => {
                    select.update(cx, |select, cx| select.set_invalid(refused, cx))
                }
                Control::OpenChoice(combobox) => {
                    combobox.update(cx, |combobox, cx| combobox.set_invalid(refused, cx))
                }
                Control::List(tags) => tags.update(cx, |tags, cx| tags.set_invalid(refused, cx)),
                Control::Date(input) => {
                    input.update(cx, |input, cx| input.set_invalid(refused, cx))
                }
                Control::Time(input) => {
                    input.update(cx, |input, cx| input.set_invalid(refused, cx))
                }
                Control::DateRange(picker) => {
                    picker.update(cx, |picker, cx| picker.set_invalid(refused, cx))
                }
                _ => {}
            }
        }
    }
}

impl Sizable for SchemaForm {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Disableable for SchemaForm {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Render for SchemaForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let count = self
            .fields
            .iter()
            .filter(|field| self.field_is_visible(field))
            .count();
        let strings = cx.strings();
        let (unrenderable_count, unrenderable_required) = self.visible_unrenderable(cx);
        let summary = (unrenderable_count > 0).then(|| {
            strings.format_plural(
                StringKey::SchemaUnrenderableOne,
                StringKey::SchemaUnrenderableMany,
                cx.numbers().plural(unrenderable_count),
                &[cx.numbers().count(unrenderable_count).as_ref()],
            )
        });
        let busy = self.validation_is_busy(cx);
        let invalid = self.validation_is_invalid(cx);
        let validation = match self.validation.clone() {
            Some(ValidationState::Validating) => {
                let validation_text = strings.text(StringKey::Validating);
                Some(
                    text(&theme, TypeScale::Caption, validation_text.clone())
                        .text_tone(&theme, TextTone::Muted)
                        .semantic_in(
                            cx,
                            NodeSpec::new(
                                self.ident.child("validation").semantic_id(),
                                Role::Status,
                            )
                            .parent(self.ident.semantic_id())
                            .text(validation_text)
                            .busy(true),
                        )
                        .into_any_element(),
                )
            }
            Some(ValidationState::Invalid { reason }) => {
                let ident = self.ident.child("validation");
                Some(
                    div()
                        .child(
                            Callout::new(reason.clone(), Tone::Danger).id(ident.child("callout")),
                        )
                        .semantic_in(
                            cx,
                            NodeSpec::new(ident.semantic_id(), Role::Status)
                                .parent(self.ident.semantic_id())
                                .text(reason)
                                .invalid(true),
                        )
                        .into_any_element(),
                )
            }
            Some(ValidationState::Pending | ValidationState::Valid) | None => None,
        };

        let rows: Vec<AnyElement> = self
            .fields
            .iter()
            .filter(|field| self.field_is_visible(field))
            .map(|field| self.field_element(field, cx))
            .collect();

        div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .gap_token(&theme, Space::Md)
            .children(rows)
            .children(validation)
            .when_some(summary, |element, summary| {
                let ident = self.ident.child("unrenderable");
                element.child(
                    div()
                        .child(
                            Callout::new(
                                summary.clone(),
                                if unrenderable_required {
                                    Tone::Danger
                                } else {
                                    Tone::Warning
                                },
                            )
                            .id(ident.child("callout")),
                        )
                        .semantic_in(
                            cx,
                            NodeSpec::new(ident.semantic_id(), Role::Status)
                                .parent(self.ident.semantic_id())
                                .text(summary)
                                .invalid(true)
                                .required(unrenderable_required)
                                .value(if unrenderable_required {
                                    "unrenderable, required"
                                } else {
                                    "unrenderable"
                                }),
                        ),
                )
            })
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Form)
                    .value(cx.numbers().count(count))
                    .busy(busy)
                    .invalid(invalid),
            )
    }
}

impl SchemaForm {
    fn field_element(&self, field: &Field, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        let ident = self.ident.child(field.path.as_ref());
        if let Control::Group = field.control {
            // A heading with a rule under it says a section starts here. An
            // indent said it too, and said it by putting the group's own
            // fields at an x nothing else in the form shares.
            return div()
                .column()
                .w_full()
                .gap_token(&theme, Space::Xs)
                .child(text(&theme, TypeScale::Subtitle, field.label.clone()))
                .when_some(field.description.clone(), |element, description| {
                    element.child(
                        text(&theme, TypeScale::Body, description)
                            .text_tone(&theme, TextTone::Muted),
                    )
                })
                .child(rule(&theme))
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Group)
                        .parent(self.ident.semantic_id())
                        .text(field.label.clone())
                        .required(field.required)
                        .level(field.level),
                )
                .into_any_element();
        }

        let control_ident = ident.child("control");
        let validation = self.validation_for(field, cx);
        let invalid = validation.is_invalid();
        let mut form_field = FormField::new(ident.clone(), field.label.clone())
            .control(control_ident.semantic_id())
            .required(field.required)
            .validation(validation);
        if let Some(description) = field.description.clone() {
            form_field = form_field.description(description);
        }
        let body: AnyElement = match &field.control {
            Control::Text(input) => input.clone().into_any_element(),
            Control::Number(number) => number.clone().into_any_element(),
            Control::Choice(select) => select.clone().into_any_element(),
            Control::OpenChoice(combobox) => combobox.clone().into_any_element(),
            Control::List(tags) => tags.clone().into_any_element(),
            Control::Date(input) => input.clone().into_any_element(),
            Control::Time(input) => input.clone().into_any_element(),
            Control::DateRange(picker) => picker.clone().into_any_element(),
            Control::Files(files) => {
                let form = cx.entity().downgrade();
                let drop_path = field.path.clone();
                let dropzone = Dropzone::new(
                    control_ident.child("dropzone"),
                    cx.strings().text(StringKey::SchemaFilesDrop),
                )
                .hint(cx.strings().text(StringKey::SchemaFilesDropHint))
                .invalid(invalid)
                .disabled(self.disabled)
                .when(!self.disabled, |dropzone| {
                    dropzone.on_files(move |external, _, cx| {
                        let paths = external.paths().to_vec();
                        let path = drop_path.clone();
                        form.update(cx, |form, cx| {
                            let _ = form.apply_files(&path, paths, true, cx);
                        })
                        .ok();
                    })
                });

                let form = cx.entity().downgrade();
                let request = files.request.clone();
                let choose = Button::new(control_ident.child("choose"))
                    .label(cx.strings().text(StringKey::SchemaFilesChoose))
                    .secondary()
                    .disabled(self.disabled)
                    .when(!self.disabled, |button| {
                        button.on_click(move |_, cx| {
                            let request = request.clone();
                            form.update(cx, |_, cx| {
                                cx.emit(SchemaFormEvent::FilesRequested(request));
                            })
                            .ok();
                        })
                    });

                let selected = files.selected.iter().map(|selected| {
                    let form = cx.entity().downgrade();
                    let path = field.path.clone();
                    let id = selected.id;
                    div()
                        .id(control_ident
                            .child("file")
                            .child(id.to_string())
                            .element_id())
                        .row_reading(direction)
                        .items_center()
                        .justify_between()
                        .gap_token(&theme, Space::Sm)
                        // An acquired file is a thing the form now holds, so
                        // it is drawn as one rather than as a line of prose
                        // with a bin beside it.
                        .ps(direction, px(theme.space(Space::Sm)))
                        .pe(direction, px(theme.space(Space::Xs)))
                        .py(px(theme.space(Space::Xs)))
                        .radius(&theme, gpui_kit_theme::Radius::Control)
                        .surface(&theme, Surface::Raised)
                        .child(
                            text(&theme, TypeScale::Body, selected.label.clone())
                                .overflow_hidden()
                                .text_ellipsis(),
                        )
                        .child(
                            IconButton::new(
                                control_ident
                                    .child("file")
                                    .child(id.to_string())
                                    .child("remove"),
                                Icon::Trash,
                                cx.strings().text(StringKey::SchemaFilesRemove),
                            )
                            .small()
                            .disabled(self.disabled)
                            .when(!self.disabled, |button| {
                                button.on_click(move |_, cx| {
                                    form.update(cx, |form, cx| form.remove_file(&path, id, cx))
                                        .ok();
                                })
                            }),
                        )
                });

                div()
                    .column()
                    .gap_token(&theme, Space::Sm)
                    .child(dropzone)
                    // The picker is the second way to do what the zone does,
                    // not part of its frame: full width and flush against it,
                    // the two read as one control with a lid.
                    .child(div().row_reading(direction).justify_end().child(choose))
                    .children(selected)
                    .children(files.refusal.clone().map(|refusal| {
                        Callout::new(refusal, Tone::Danger).id(control_ident.child("refusal"))
                    }))
                    .semantic_in(
                        cx,
                        NodeSpec::new(control_ident.semantic_id(), Role::Group)
                            .disabled(self.disabled)
                            .invalid(invalid),
                    )
                    .into_any_element()
            }
            Control::Repeated(repeated) => {
                let item_count = repeated.items.len();
                let items = repeated.items.iter().enumerate().map(|(index, item)| {
                    let item_ident = control_ident.child(format!("item-{}", item.id));

                    let form = cx.entity().downgrade();
                    let path = field.path.clone();
                    let move_up = IconButton::new(
                        item_ident.child("move-up"),
                        Icon::ArrowUp,
                        cx.strings().text(StringKey::SchemaListMoveUp),
                    )
                    .small()
                    .disabled(self.disabled || index == 0)
                    .when(!self.disabled && index > 0, |button| {
                        button.on_click(move |_, cx| {
                            form.update(cx, |form, cx| {
                                form.move_list_item(path.clone(), index, index - 1, cx);
                            })
                            .ok();
                        })
                    });

                    let form = cx.entity().downgrade();
                    let path = field.path.clone();
                    let move_down = IconButton::new(
                        item_ident.child("move-down"),
                        Icon::ArrowDown,
                        cx.strings().text(StringKey::SchemaListMoveDown),
                    )
                    .small()
                    .disabled(self.disabled || index + 1 == item_count)
                    .when(!self.disabled && index + 1 < item_count, |button| {
                        button.on_click(move |_, cx| {
                            form.update(cx, |form, cx| {
                                form.move_list_item(path.clone(), index, index + 1, cx);
                            })
                            .ok();
                        })
                    });

                    let form = cx.entity().downgrade();
                    let path = field.path.clone();
                    let remove = IconButton::new(
                        item_ident.child("remove"),
                        Icon::Trash,
                        cx.strings().text(StringKey::SchemaListRemove),
                    )
                    .small()
                    .disabled(self.disabled)
                    .when(!self.disabled, |button| {
                        button.on_click(move |_, cx| {
                            form.update(cx, |form, cx| {
                                form.remove_list_item(path.clone(), index, cx);
                            })
                            .ok();
                        })
                    });

                    let number = cx.numbers().count(index + 1);
                    let label = cx
                        .strings()
                        .format(StringKey::SchemaListItem, &[number.as_ref()]);
                    div()
                        .id(item_ident.element_id())
                        .column()
                        .gap_token(&theme, Space::Sm)
                        .p_token(&theme, Space::Sm)
                        .border(px(theme.borders.hairline))
                        .border_color(theme.colors.hairline)
                        .radius(&theme, gpui_kit_theme::Radius::Card)
                        .child(
                            div()
                                .row_reading(direction)
                                .items_center()
                                .justify_between()
                                .child(text(&theme, TypeScale::Label, label))
                                .child(
                                    div()
                                        .row_reading(direction)
                                        .items_center()
                                        .child(move_up)
                                        .child(move_down)
                                        .child(remove),
                                ),
                        )
                        .child(item.form.clone())
                });

                let form = cx.entity().downgrade();
                let path = field.path.clone();
                let at_maximum = repeated
                    .max
                    .is_some_and(|maximum| repeated.items.len() >= maximum);
                div()
                    .column()
                    .gap_token(&theme, Space::Sm)
                    .children(items)
                    .child(
                        Button::new(control_ident.child("add"))
                            .label(cx.strings().text(StringKey::SchemaListAdd))
                            .icon(Icon::Plus)
                            .secondary()
                            .disabled(self.disabled || at_maximum)
                            .when(!self.disabled && !at_maximum, |button| {
                                button.on_click(move |window, cx| {
                                    form.update(cx, |form, cx| {
                                        form.add_list_item(path.clone(), window, cx);
                                    })
                                    .ok();
                                })
                            }),
                    )
                    .into_any_element()
            }
            Control::Boolean(on) => {
                let on = *on;
                let path = field.path.clone();
                // A switch takes effect at once, so the draft it moves lives
                // here rather than in a control that would report and forget.
                let form = cx.entity().downgrade();
                Switch::new(control_ident.clone())
                    // The visible label belongs to the field around it, so the
                    // switch carries the same name rather than going unnamed.
                    .named(field.label.clone())
                    .on(on)
                    .invalid(invalid)
                    .disabled(self.disabled)
                    .when(!self.disabled, |switch| {
                        switch.on_change(move |next, _, cx| {
                            let path = path.clone();
                            form.update(cx, |form, cx| {
                                if let Some(field) =
                                    form.fields.iter_mut().find(|field| field.path == path)
                                {
                                    field.control = Control::Boolean(next);
                                }
                                form.changed(path, cx);
                            })
                            .ok();
                        })
                    })
                    .into_any_element()
            }
            // The field keeps its place, its label, and its required mark. It
            // is the control that is missing, and the reason stands where the
            // control would have been.
            Control::Unrenderable(reason) => {
                let refusal = ident.child("unrenderable");
                div()
                    .child(
                        Callout::new(
                            reason.clone(),
                            if field.required {
                                Tone::Danger
                            } else {
                                Tone::Warning
                            },
                        )
                        .id(refusal.child("callout")),
                    )
                    .semantic_in(
                        cx,
                        NodeSpec::new(refusal.semantic_id(), Role::Status)
                            .parent(ident.semantic_id())
                            .text(reason.clone())
                            .invalid(true)
                            .required(field.required)
                            .value(if field.required {
                                "unrenderable, required"
                            } else {
                                "unrenderable"
                            }),
                    )
                    .into_any_element()
            }
            Control::Group => div().into_any_element(),
        };

        div().child(form_field.child(body)).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_shows_its_name_when_the_schema_named_nothing_else() {
        let field = SchemaField::new("max_tokens", SchemaKind::Integer(NumberBounds::new()));
        assert_eq!(field.shown_label().as_ref(), "max_tokens");
        assert_eq!(
            field.label("Maximum tokens").shown_label().as_ref(),
            "Maximum tokens"
        );
    }

    #[test]
    fn bounds_are_absent_until_a_schema_states_them() {
        let bounds = NumberBounds::new();
        assert_eq!(bounds.min, None);
        assert_eq!(bounds.max, None);
        let bounded = NumberBounds::new().min(1.0).max(4.0).step(0.5);
        assert_eq!(bounded.min, Some(1.0));
        assert_eq!(bounded.max, Some(4.0));
        assert_eq!(bounded.step, Some(0.5));
    }
}
