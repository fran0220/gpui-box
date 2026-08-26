//! A caller-owned set of text fields, and the rules the caller judges them by.
//!
//! A form does not render. It holds one [`Signal<String>`] per field, the
//! rules the caller attached, and the [`ValidationState`] each field is
//! currently in — the same ladder every field control already publishes, so a
//! form result and a host-driven result are the same vocabulary.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::{Rc, Weak};

use gpui::{App, Context, SharedString, Subscription};

use crate::reactive::Signal;
use crate::state::ValidationState;

/// What a rule is given and what it may answer.
///
/// The field's own text arrives first, the whole form second, so a rule that
/// only looks at one field ignores the second argument and a rule that
/// compares two fields does not need a second mechanism. `&App` is there
/// because the reason a rule gives is text a reader reads, and every word
/// this library shows comes from the installed
/// [`Strings`](crate::strings::Strings) catalogue.
pub type Rule = Rc<dyn Fn(&str, &FormValues, &App) -> Result<(), SharedString>>;

/// Every field's current text, by name.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct FormValues {
    values: BTreeMap<SharedString, String>,
}

impl std::fmt::Debug for FormValues {
    /// A form field holds what somebody typed, including a credential, so
    /// only the field names are printed.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_set()
            .entries(self.values.keys().map(SharedString::as_ref))
            .finish()
    }
}

impl FormValues {
    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    /// The text of a field, and the empty string for one that is not there.
    pub fn value(&self, name: &str) -> &str {
        self.get(name).unwrap_or_default()
    }

    pub fn names(&self) -> impl Iterator<Item = &SharedString> {
        self.values.keys()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// The same values, in the plain map a host submits.
    pub fn into_map(self) -> BTreeMap<String, String> {
        self.values
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect()
    }
}

#[derive(Clone)]
struct Field {
    value: Signal<String>,
    validation: Signal<ValidationState>,
    rules: Vec<Rule>,
}

#[derive(Default)]
struct Fields {
    order: Vec<SharedString>,
    fields: HashMap<SharedString, Field>,
    /// Installed the first time a form is submitted, so a field a reader is
    /// correcting stops being wrong the moment it stops being wrong.
    live: Vec<Subscription>,
}

/// A set of named text fields and the rules that judge them.
///
/// The caller builds one, keeps it, and hands each field's signal to whatever
/// control edits it. The form never renders and never decides what a failed
/// submission looks like: it records a [`ValidationState`] per field and the
/// caller draws it.
///
/// ```no_run
/// # use gpui::App;
/// # use gpui_kit::reactive::{Form, validators};
/// # fn example(cx: &mut App) {
/// let form = Form::new()
///     .field(cx, "email", "")
///     .field(cx, "password", "")
///     .field(cx, "confirm", "")
///     .rule("email", validators::required())
///     .rule("email", validators::email())
///     .rule("password", validators::min_len(12))
///     .rule("confirm", validators::equals_field("password"));
///
/// if let Some(values) = form.submit(cx) {
///     let _ = values;
/// }
/// # }
/// ```
#[derive(Clone, Default)]
pub struct Form {
    inner: Rc<RefCell<Fields>>,
}

impl std::fmt::Debug for Form {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Form")
            .field("fields", &self.names())
            .finish()
    }
}

impl Form {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a field with its starting text.
    ///
    /// Adding a name that is already there replaces nothing: the existing
    /// field and everything already bound to it stay as they are.
    pub fn field(
        self,
        cx: &mut App,
        name: impl Into<SharedString>,
        initial: impl Into<String>,
    ) -> Self {
        let name = name.into();
        let mut inner = self.inner.borrow_mut();
        if !inner.fields.contains_key(&name) {
            let field = Field {
                value: Signal::new(cx, initial.into()),
                validation: Signal::new(cx, ValidationState::Pending),
                rules: Vec::new(),
            };
            inner.order.push(name.clone());
            inner.fields.insert(name, field);
        }
        drop(inner);
        self
    }

    /// Attaches one rule to one field. A field may carry several, and they
    /// are run in the order they were attached.
    ///
    /// A rule named against a field that does not exist is dropped, because
    /// there is nothing for it to judge.
    pub fn rule(
        self,
        name: &str,
        rule: impl Fn(&str, &FormValues, &App) -> Result<(), SharedString> + 'static,
    ) -> Self {
        {
            let mut inner = self.inner.borrow_mut();
            debug_assert!(inner.fields.contains_key(name));
            if let Some(field) = inner.fields.get_mut(name) {
                field.rules.push(Rc::new(rule));
            }
        }
        self
    }

    /// Every field name, in the order the fields were added.
    pub fn names(&self) -> Vec<SharedString> {
        self.inner.borrow().order.clone()
    }

    /// The signal holding one field's text, which is what a control binds to.
    pub fn signal(&self, name: &str) -> Option<Signal<String>> {
        self.inner
            .borrow()
            .fields
            .get(name)
            .map(|field| field.value.clone())
    }

    /// The signal holding one field's validation state, for a view that wants
    /// to watch it directly.
    pub fn validation_signal(&self, name: &str) -> Option<Signal<ValidationState>> {
        self.inner
            .borrow()
            .fields
            .get(name)
            .map(|field| field.validation.clone())
    }

    /// What is currently known about one field. A field nobody has judged,
    /// and a name that is not a field, are both
    /// [`ValidationState::Pending`].
    pub fn validation(&self, name: &str, cx: &App) -> ValidationState {
        self.inner
            .borrow()
            .fields
            .get(name)
            .map(|field| field.validation.get(cx))
            .unwrap_or_default()
    }

    /// Records a state the host decided, such as
    /// [`ValidationState::Validating`] while a check is in flight.
    ///
    /// A field left in `Validating` is never overwritten by a rule: the host
    /// owns it until the host says otherwise.
    pub fn set_validation(&self, cx: &mut App, name: &str, state: ValidationState) {
        let validation = self
            .inner
            .borrow()
            .fields
            .get(name)
            .map(|field| field.validation.clone());
        if let Some(validation) = validation {
            validation.set(cx, state);
        }
    }

    pub fn values(&self, cx: &App) -> FormValues {
        let inner = self.inner.borrow();
        FormValues {
            values: inner
                .order
                .iter()
                .filter_map(|name| {
                    let field = inner.fields.get(name)?;
                    Some((name.clone(), field.value.get(cx)))
                })
                .collect(),
        }
    }

    /// Runs every rule and records what each field is now known to be.
    ///
    /// Returns whether every field came out [`ValidationState::Valid`]. A
    /// field the host left `Validating` is not valid and is not invalid: it
    /// is unfinished, so the answer is `false` and nothing about it changes.
    pub fn validate(&self, cx: &mut App) -> bool {
        judge(&self.inner, cx, Judge::Every)
    }

    /// Validates, and answers with the values only when every field passed.
    ///
    /// After a submission, every judged field is re-validated as it is
    /// edited, so a reason a reader has fixed disappears without another
    /// submission.
    pub fn submit(&self, cx: &mut App) -> Option<BTreeMap<String, String>> {
        let passed = self.validate(cx);
        self.watch_edits(cx);
        passed.then(|| self.values(cx).into_map())
    }

    /// Re-renders the watching view whenever any field's text or validation
    /// state changes.
    ///
    /// The returned subscriptions are the watch: a view keeps them for as
    /// long as it draws the form.
    #[must_use]
    pub fn watch<V: 'static>(&self, cx: &mut Context<V>) -> Vec<Subscription> {
        let inner = self.inner.borrow();
        inner
            .order
            .iter()
            .filter_map(|name| inner.fields.get(name))
            .flat_map(|field| [field.value.watch(cx), field.validation.watch(cx)])
            .collect()
    }

    /// Installs the live re-validation that follows a submission.
    fn watch_edits(&self, cx: &mut App) {
        if !self.inner.borrow().live.is_empty() {
            return;
        }
        let signals: Vec<Signal<String>> = {
            let inner = self.inner.borrow();
            inner
                .order
                .iter()
                .filter_map(|name| inner.fields.get(name))
                .map(|field| field.value.clone())
                .collect()
        };
        let weak = Rc::downgrade(&self.inner);
        let live: Vec<Subscription> = signals
            .iter()
            .map(|signal| {
                let weak: Weak<RefCell<Fields>> = weak.clone();
                cx.observe(signal.entity(), move |_, cx| {
                    if let Some(inner) = weak.upgrade() {
                        judge(&inner, cx, Judge::AlreadyJudged);
                    }
                })
            })
            .collect();
        self.inner.borrow_mut().live = live;
    }
}

/// Which fields a pass of the rules is allowed to change.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Judge {
    /// A submission: everything the host is not still checking.
    Every,
    /// An edit after a submission: only fields that already carry an answer,
    /// so a field nobody has reached is not marked wrong for being empty.
    AlreadyJudged,
}

fn judge(inner: &Rc<RefCell<Fields>>, cx: &mut App, which: Judge) -> bool {
    // The rules are run against a copy taken up front, because a rule reads
    // the whole form and running one must not hold a borrow of it.
    let plan: Vec<(SharedString, Field)> = {
        let fields = inner.borrow();
        fields
            .order
            .iter()
            .filter_map(|name| Some((name.clone(), fields.fields.get(name)?.clone())))
            .collect()
    };

    let values = FormValues {
        values: plan
            .iter()
            .map(|(name, field)| (name.clone(), field.value.get(cx)))
            .collect(),
    };

    let mut all_valid = true;
    for (name, field) in &plan {
        let (validation, rules) = (&field.validation, &field.rules);
        let current = validation.get(cx);
        // A check the host has in flight is the host's answer to give.
        if current.is_busy() {
            all_valid = false;
            continue;
        }
        if which == Judge::AlreadyJudged && current == ValidationState::Pending {
            all_valid = false;
            continue;
        }
        let text = values.value(name);
        let outcome = rules
            .iter()
            .try_fold((), |(), rule| rule(text, &values, cx))
            .err();
        let state = match outcome {
            Some(reason) => {
                all_valid = false;
                ValidationState::Invalid { reason }
            }
            None => ValidationState::Valid,
        };
        validation.set(cx, state);
    }
    all_valid
}

/// The rules a form is most often built out of.
///
/// Each returns a closure that fits [`Rule`], and each takes its reason from
/// the installed [`Strings`](crate::strings::Strings) catalogue, so a host
/// that replaced those words gets its own words back out of a validation
/// failure.
pub mod validators {
    use gpui::{App, SharedString};

    use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

    use super::FormValues;

    /// Refuses text that is empty, or is only whitespace.
    pub fn required() -> impl Fn(&str, &FormValues, &App) -> Result<(), SharedString> {
        |value: &str, _values: &FormValues, cx: &App| {
            if value.trim().is_empty() {
                return Err(cx.strings().text(StringKey::FormRequired));
            }
            Ok(())
        }
    }

    /// Refuses text that is not shaped like an address.
    ///
    /// The shape is deliberately the least this library can check — one `@`
    /// with something either side of it and a dot after it — because whether
    /// an address exists is a question for the host, not for a text field.
    /// Empty text passes: an optional field is not an invalid one, and
    /// [`required`] is how a caller says otherwise.
    pub fn email() -> impl Fn(&str, &FormValues, &App) -> Result<(), SharedString> {
        |value: &str, _values: &FormValues, cx: &App| {
            let value = value.trim();
            if value.is_empty() {
                return Ok(());
            }
            let mut parts = value.split('@');
            let local = parts.next().unwrap_or_default();
            let domain = parts.next().unwrap_or_default();
            let shaped = parts.next().is_none()
                && !local.is_empty()
                && domain.split('.').filter(|label| !label.is_empty()).count() >= 2
                && !domain.starts_with('.')
                && !domain.ends_with('.')
                && !value.contains(char::is_whitespace);
            if shaped {
                Ok(())
            } else {
                Err(cx.strings().text(StringKey::FormEmail))
            }
        }
    }

    /// Refuses text shorter than `least` characters, counted the way a reader
    /// counts them rather than in bytes.
    pub fn min_len(least: usize) -> impl Fn(&str, &FormValues, &App) -> Result<(), SharedString> {
        move |value: &str, _values: &FormValues, cx: &App| {
            if value.chars().count() >= least {
                return Ok(());
            }
            let digits = cx.numbers().count(least);
            Err(cx.strings().format_plural(
                StringKey::FormMinLengthOne,
                StringKey::FormMinLengthMany,
                cx.numbers().plural(least),
                &[digits.as_ref()],
            ))
        }
    }

    /// Refuses text that differs from another field's text.
    pub fn equals_field(
        other: impl Into<SharedString>,
    ) -> impl Fn(&str, &FormValues, &App) -> Result<(), SharedString> {
        let other = other.into();
        move |value: &str, values: &FormValues, cx: &App| {
            if values.value(other.as_ref()) == value {
                Ok(())
            } else {
                Err(cx.strings().text(StringKey::FormFieldsDiffer))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;

    use crate::strings::StringKey;

    use super::*;

    fn form(cx: &mut App) -> Form {
        Form::new()
            .field(cx, "email", "")
            .field(cx, "password", "")
            .field(cx, "confirm", "")
            .rule("email", validators::required())
            .rule("email", validators::email())
            .rule("password", validators::min_len(8))
            .rule("confirm", validators::equals_field("password"))
    }

    #[gpui::test]
    fn an_untouched_form_has_judged_nothing(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let form = form(cx);
            for name in form.names() {
                assert_eq!(form.validation(&name, cx), ValidationState::Pending);
            }
        });
    }

    #[gpui::test]
    fn a_failing_rule_records_its_own_reason(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let form = form(cx);
            assert!(form.submit(cx).is_none());
            assert_eq!(
                form.validation("email", cx),
                ValidationState::invalid(StringKey::FormRequired.english())
            );
            assert_eq!(
                form.validation("password", cx),
                ValidationState::invalid("Use at least 8 characters.")
            );
            // Two empty fields match each other, so the cross-field rule holds.
            assert_eq!(form.validation("confirm", cx), ValidationState::Valid);
        });
    }

    #[gpui::test]
    fn an_errored_field_stops_being_wrong_as_it_is_corrected(cx: &mut TestAppContext) {
        let form = cx.update(|cx| {
            let form = form(cx);
            assert!(form.submit(cx).is_none());
            form
        });

        let email = form.signal("email").expect("email");
        cx.update(|cx| email.set(cx, String::from("not-an-address")));
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(
                form.validation("email", cx),
                ValidationState::invalid(StringKey::FormEmail.english()),
                "the reason changed to the one that now applies"
            );
        });

        cx.update(|cx| email.set(cx, String::from("ada@example.com")));
        cx.run_until_parked();
        cx.update(|cx| assert_eq!(form.validation("email", cx), ValidationState::Valid));
    }

    #[gpui::test]
    fn a_cross_field_rule_reads_the_other_field(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let form = form(cx);
            form.signal("password")
                .expect("password")
                .set(cx, String::from("correct horse"));
            form.signal("confirm")
                .expect("confirm")
                .set(cx, String::from("correct hors"));
            assert!(form.submit(cx).is_none());
            assert_eq!(
                form.validation("confirm", cx),
                ValidationState::invalid(StringKey::FormFieldsDiffer.english())
            );
        });
    }

    #[gpui::test]
    fn a_passing_submission_answers_with_every_value(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let form = form(cx);
            form.signal("email")
                .expect("email")
                .set(cx, String::from("ada@example.com"));
            form.signal("password")
                .expect("password")
                .set(cx, String::from("correct horse"));
            form.signal("confirm")
                .expect("confirm")
                .set(cx, String::from("correct horse"));

            let values = form.submit(cx).expect("a form nothing refused");
            assert_eq!(values.len(), 3);
            assert_eq!(values["email"], "ada@example.com");
            assert_eq!(values["confirm"], "correct horse");
            for name in form.names() {
                assert_eq!(form.validation(&name, cx), ValidationState::Valid);
            }
        });
    }

    #[gpui::test]
    fn a_check_the_host_is_running_is_not_overwritten(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let form = form(cx);
            form.set_validation(cx, "email", ValidationState::Validating);
            assert!(
                form.submit(cx).is_none(),
                "an unfinished check is not a pass"
            );
            assert_eq!(form.validation("email", cx), ValidationState::Validating);
        });
    }

    #[gpui::test]
    fn form_values_never_print_what_was_typed(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let form = form(cx);
            form.signal("password")
                .expect("password")
                .set(cx, String::from("hunter2"));
            let printed = format!("{:?}", form.values(cx));
            assert!(!printed.contains("hunter2"));
            assert!(printed.contains("password"));
        });
    }
}
