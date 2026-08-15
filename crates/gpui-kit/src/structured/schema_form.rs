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
//! # Whose error is on screen
//!
//! Two sources, kept apart. [`SchemaForm::validate`] marks required fields
//! nobody filled in, which is all this component can judge on its own;
//! [`SchemaForm::set_error`] shows an error the host returned, in the host's
//! own words. A host error outranks a derived one on the same field, because
//! the host knows something the form does not.

use std::collections::BTreeMap;

use gpui::{
    AnyElement, App, AppContext, Context, Entity, EventEmitter, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, Styled, Subscription, Window, div, prelude::FluentBuilder,
    px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, TextTone, TypeScale};

use crate::controls::combobox::{Combobox, ComboboxEvent};
use crate::controls::form_field::FormField;
use crate::controls::input::{TextInput, TextInputEvent};
use crate::controls::number_input::{NumberInput, NumberInputEvent};
use crate::controls::select::{Select, SelectEvent, SelectOption};
use crate::controls::tag_input::{TagInput, TagInputEvent};
use crate::controls::toggle::Switch;
use crate::display::badge::Tone;
use crate::display::status::Callout;
use crate::foundation::{Disableable, Ident, Sizable, StyledExt, text};
use crate::strings::{ActiveStrings, StringKey};

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
    /// Paths the host already holds, collected through [`crate::controls::Dropzone`].
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
    /// Nothing was entered. Distinct from an empty string, which somebody
    /// typed on purpose.
    Absent,
    /// The form could not draw this field, so it holds nothing and never
    /// could. Reported rather than omitted.
    Unrenderable,
}

/// What the form reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaFormEvent {
    /// The field at this path changed. The value is read from
    /// [`SchemaForm::values`], because a form has more than one of them.
    Changed(SharedString),
    /// The primary key was pressed in a field. The form submits nothing.
    Submitted,
}

impl EventEmitter<SchemaFormEvent> for SchemaForm {}

/// The control that edits one field, and where the value lives.
enum Control {
    Text(Entity<TextInput>),
    Number(Entity<NumberInput>),
    /// A switch is a builder rather than a view, so the draft is here.
    Boolean(bool),
    Choice(Entity<Select>),
    OpenChoice(Entity<Combobox>),
    List(Entity<TagInput>),
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
    fields: Vec<Field>,
    /// What the host said is wrong, by path.
    host_errors: BTreeMap<SharedString, SharedString>,
    /// What the form worked out is missing, by path. Cleared by every edit to
    /// that field, so an answered complaint does not stay on screen.
    derived_errors: BTreeMap<SharedString, SharedString>,
    unrenderable: Vec<UnrenderableField>,
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
            fields: Vec::new(),
            host_errors: BTreeMap::new(),
            derived_errors: BTreeMap::new(),
            unrenderable: Vec::new(),
            size: ControlSize::Md,
            disabled: false,
            _subscriptions: Vec::new(),
        };
        let ident = form.ident.clone();
        form.build(&schema.fields, "", 1, &ident, window, cx);
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
            let control = self.control_for(field, &field_ident, window, cx);

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
        ident: &Ident,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Control {
        let path = SharedString::from(ident.as_str().to_string());
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
                self.watch_text(&path, &input, cx);
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
                self.watch_number(&path, &number, cx);
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
                self.watch_select(&path, &select, cx);
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
                self.watch_combobox(&path, &combobox, cx);
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
                self.watch_tags(&path, &tags, cx);
                Control::List(tags)
            }
            // These shapes are named so a host can describe a settings page
            // without dropping fields. Wiring them to DateInput / Dropzone
            // still needs a host adapter or a file policy; until that is
            // supplied the field stays visible as unrenderable.
            SchemaKind::Date | SchemaKind::Time | SchemaKind::DateRange => {
                Control::Unrenderable(cx.strings().text(StringKey::SchemaNeedsAdapter))
            }
            SchemaKind::Files { .. } | SchemaKind::List { .. } => {
                Control::Unrenderable(cx.strings().text(StringKey::SchemaNeedsHost))
            }
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

    fn changed(&mut self, path: SharedString, cx: &mut Context<Self>) {
        // The form's own complaint was about this field being empty, and it is
        // not empty any more; the host's stands until the host withdraws it.
        self.derived_errors.remove(&path);
        cx.emit(SchemaFormEvent::Changed(path));
        cx.notify();
    }

    /// Shows an error the host returned, next to the field it is about.
    pub fn set_error(
        &mut self,
        path: impl Into<SharedString>,
        message: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.host_errors.insert(path.into(), message.into());
        cx.notify();
    }

    /// Withdraws every error the host reported. What the form worked out for
    /// itself is untouched, because the host did not put it there.
    pub fn clear_host_errors(&mut self, cx: &mut Context<Self>) {
        self.host_errors.clear();
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
                field.required && matches!(self.value_of(field, cx), FieldValue::Absent)
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
        cx.notify();
        self.derived_errors.is_empty() && !self.unrenderable.iter().any(|field| field.required)
    }

    /// Every field and what it holds, including the ones the form could not
    /// draw. A caller that builds a call from this cannot lose a field without
    /// seeing it.
    pub fn values(&self, cx: &App) -> Vec<(SharedString, FieldValue)> {
        self.fields
            .iter()
            .filter(|field| !matches!(field.control, Control::Group))
            .map(|field| (field.path.clone(), self.value_of(field, cx)))
            .collect()
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
                Control::Boolean(_) | Control::Group | Control::Unrenderable(_) => {}
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
            Control::Unrenderable(_) => FieldValue::Unrenderable,
            Control::Group => FieldValue::Absent,
        }
    }

    /// The error shown on a field. The host's outranks the form's, because the
    /// host knows something the form does not.
    ///
    /// A control that draws itself as invalid is asked why last of all, so the
    /// red border a number gets for leaving its range always arrives with the
    /// range beside it rather than on its own.
    fn error_for(&self, field: &Field, cx: &App) -> Option<SharedString> {
        if let Some(error) = self
            .host_errors
            .get(&field.path)
            .or_else(|| self.derived_errors.get(&field.path))
        {
            return Some(error.clone());
        }
        match &field.control {
            Control::Number(number) => number.read(cx).invalid_reason(cx),
            _ => None,
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
        let count = self.fields.len();
        let strings = cx.strings();
        let unrenderable_required = self.has_unrenderable_required();
        let summary = (!self.unrenderable.is_empty()).then(|| {
            if self.unrenderable.len() == 1 {
                strings.text(StringKey::SchemaUnrenderableOne)
            } else {
                strings.format(
                    StringKey::SchemaUnrenderableMany,
                    &[&self.unrenderable.len().to_string()],
                )
            }
        });

        let rows: Vec<AnyElement> = self
            .fields
            .iter()
            .map(|field| self.field_element(field, cx))
            .collect();

        div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .gap_token(&theme, Space::Md)
            .children(rows)
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
                NodeSpec::new(self.ident.semantic_id(), Role::Form).value(count.to_string()),
            )
    }
}

impl SchemaForm {
    fn field_element(&self, field: &Field, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let ident = self.ident.child(field.path.as_ref());
        let indent = px(field.level.saturating_sub(1) as f32 * theme.space(Space::Lg));

        if let Control::Group = field.control {
            return div()
                .ml(indent)
                .column()
                .gap_token(&theme, Space::Xs)
                .child(text(&theme, TypeScale::Subtitle, field.label.clone()))
                .when_some(field.description.clone(), |element, description| {
                    element.child(
                        text(&theme, TypeScale::Body, description)
                            .text_tone(&theme, TextTone::Muted),
                    )
                })
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
        let error = self.error_for(field, cx);
        let mut form_field = FormField::new(ident.clone(), field.label.clone())
            .control(control_ident.semantic_id())
            .required(field.required);
        if let Some(description) = field.description.clone() {
            form_field = form_field.description(description);
        }
        if let Some(error) = error.clone() {
            form_field = form_field.error(error);
        }

        let body: AnyElement = match &field.control {
            Control::Text(input) => input.clone().into_any_element(),
            Control::Number(number) => number.clone().into_any_element(),
            Control::Choice(select) => select.clone().into_any_element(),
            Control::OpenChoice(combobox) => combobox.clone().into_any_element(),
            Control::List(tags) => tags.clone().into_any_element(),
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

        div()
            .ml(indent)
            .child(form_field.child(body))
            .into_any_element()
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
