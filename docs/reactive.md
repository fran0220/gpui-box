# Reactive state

`gpui_kit::reactive` is caller-owned state and the wiring that connects it to
a control. Nothing in it renders, and no component owns any of it.

| Type | What it is |
|---|---|
| `Signal<T>` | A value the caller creates and keeps. Changing it notifies every watcher once. |
| `Binding<T>` | A read and a write of one value, handed to a control. Not storage. |
| `Form` | Named `Signal<String>` fields, the caller's rules, and a `ValidationState` per field. |
| `validators` | `required`, `email`, `min_len`, `equals_field` — the rules most forms are built from. |

A bound control is still a reader of caller data. `.bind` sets the same value
and the same handler the caller would have written by hand, so a caller that
refuses a change still sees the control keep showing what is true.

## A signal

```rust
let volume = Signal::new(cx, 0.6f32);

volume.get(cx);                         // 0.6
volume.update(cx, |value| *value += 0.1);   // notifies
volume.set(cx, 0.7);                    // notifies nobody: it is already 0.7
```

A view watches it, and keeps the subscription for as long as it draws it:

```rust
struct Mixer {
    volume: Signal<f32>,
    _watch: Subscription,
}

impl Mixer {
    fn new(volume: Signal<f32>, cx: &mut Context<Self>) -> Self {
        Self {
            _watch: volume.watch(cx),
            volume,
        }
    }
}
```

`set` says nothing when the value did not move. That is the whole echo guard:
a control reports a change, the change writes the signal, and the signal
writing the control back is what would move the caret or fire the handler
again.

## A binding

```rust
let binding = volume.binding();
binding.get(cx);
binding.set(cx, 0.4);
```

`map` converts in both directions, and `lens` projects one field of a struct:

```rust
let percent: Binding<f64> = volume
    .binding()
    .map(|value| f64::from(*value) * 100.0, |value, percent| {
        *value = (percent / 100.0) as f32
    });

let name: Binding<String> = profile.lens(
    |profile| &profile.name,
    |profile, value| profile.name = value,
);
```

A lens write is a read-modify-write of the whole value, so it moves the field
it projects and leaves every other field exactly as it was.

## Binding a control

Builders take a `Binding` and a context, because the current value is read at
build time:

```rust
Checkbox::new("terms.accept").label("Accept the terms").bind(&accepted.binding(), cx)
Switch::new("run.notify").bind(&notify.binding(), cx)
Toggle::new("format.bold").icon(Icon::Bold).bind(&bold.binding(), cx)
ToggleGroup::new("format.marks").items(marks).bind(&pressed.binding(), cx)
SegmentedControl::new("view.mode").segments(modes).bind(&mode.binding(), cx)
Slider::new("mixer.volume").bind(&volume.binding(), cx)
Radio::new("plan.monthly").bind_value(&plan.binding(), Plan::Monthly, cx)
```

A radio binds a *value* rather than a flag: every button in the group binds
the same binding with its own value, and the exclusivity is the equality.

Views are entities, so they take the signal and answer with the subscriptions
that are the binding. The caller holds them for as long as the two should
stay together:

```rust
let subscriptions = TextInput::bind(&email_field, &email, cx);
```

| Control | Signal | Reported by |
|---|---|---|
| `TextInput` | `Signal<String>` | `TextInputEvent::Change` |
| `TextArea` | `Signal<String>` | `TextAreaEvent::Change` |
| `PasswordInput` | `Signal<String>` | `PasswordInputEvent::Change` |
| `NumberInput` | `Signal<f64>` | `NumberInputEvent::Changed` |
| `Select` | `Signal<Option<SharedString>>` | `SelectEvent::Selected`, `Cleared` |
| `TagInput` | `Signal<Vec<SharedString>>` | `TagInputEvent::Added`, `Removed`, `Moved` |

Two of those are worth reading twice.

A `NumberInput` that holds text which is not a number writes nothing: the
signal keeps the last number that was one and the field keeps showing what was
typed, so the disagreement stays visible instead of being resolved by guessing.
A value outside the range is still a number and is still written.

A `TagInput` reports what the typist asked for and never applies it, so `bind`
is what applying it looks like. A duplicate and a full field are refusals — the
field is already saying so where the typist is looking — and the set does not
change.

## A form

Every field is its own `Signal<String>`. Rules are the caller's, they run
synchronously, and each is given the field's own text and the whole form, so a
rule that compares two fields needs no second mechanism.

```rust
let form = Form::new()
    .field(cx, "email", "")
    .field(cx, "password", "")
    .field(cx, "confirm", "")
    .rule("email", validators::required())
    .rule("email", validators::email())
    .rule("password", validators::min_len(12))
    .rule("confirm", validators::equals_field("password"));

let email = form.signal("email").expect("a field that was added");
let subscriptions = TextInput::bind(&email_field, &email, cx);

if let Some(values) = form.submit(cx) {
    host.sign_in(values);
}
```

A submission that fails answers `None` and records why, per field, on the
[`ValidationState`](../crates/gpui-kit/src/state.rs) ladder every field control
already publishes:

```rust
match form.validation("email", cx) {
    ValidationState::Pending => {}
    ValidationState::Validating => {}
    ValidationState::Invalid { reason } => field.invalid(true).message(reason),
    ValidationState::Valid => {}
}
```

There is no second vocabulary for a form result. `Pending` is a field nobody
has judged, which is what an untouched form is made of.

After a submission, every judged field is re-validated as it is edited, so a
reason a reader has fixed disappears without another submission. A field that
was never judged is left alone: a field nobody has reached is not marked wrong
for being empty.

### Asynchronous checks belong to the host

Rules here are synchronous. A check that has to leave the machine is the
host's, and the host says so on the same ladder:

```rust
form.set_validation(cx, "email", ValidationState::Validating);
// … later
form.set_validation(cx, "email", ValidationState::invalid(refusal));
```

A field left in `Validating` is never overwritten by a rule and never counts
as a pass. An unfinished check is not a failure and is not a success.

### The words a rule gives

A validator's reason comes from the installed
[`Strings`](../crates/gpui-kit/src/strings.rs) catalogue —
`FormRequired`, `FormEmail`, `FormMinLengthOne` / `FormMinLengthMany`,
`FormFieldsDiffer` — so a host that replaced those words gets its own words
back out of a validation failure. A rule the caller writes returns whatever
text the caller wants; nothing here authors it.

`validators::email` checks the least this library can check: one `@`, something
either side of it, and a dot in the domain. Whether an address exists is a
question for the host. Empty text passes it, because an optional field is not
an invalid one and `required` is how a caller says otherwise.

## What is not printed

A `Signal` prints its identity and never its value, and `FormValues` prints
field names and never what was typed. A form field holds what somebody typed,
and a credential is one of the things somebody types.
