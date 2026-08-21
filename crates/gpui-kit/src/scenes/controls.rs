//! Things a pointer or a keyboard acts on directly.

use super::support::*;

pub(super) fn button(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .child(
            row(&theme)
                .child(
                    Button::new("scene.button.primary")
                        .label("Primary")
                        .primary()
                        .on_click(|_, _| {}),
                )
                .child(
                    Button::new("scene.button.secondary")
                        .label("Secondary")
                        .secondary()
                        .on_click(|_, _| {}),
                )
                .child(
                    Button::new("scene.button.ghost")
                        .label("Ghost")
                        .ghost()
                        .on_click(|_, _| {}),
                )
                .child(
                    Button::new("scene.button.danger")
                        .label("Delete")
                        .danger()
                        .on_click(|_, _| {}),
                )
                .child(
                    Button::new("scene.button.link")
                        .label("Learn more")
                        .link()
                        .on_click(|_, _| {}),
                ),
        )
        .child(
            row(&theme)
                .child(
                    Button::new("scene.button.disabled")
                        .label("Unavailable")
                        .primary()
                        .disabled(true)
                        .on_click(|_, _| {}),
                )
                .child(
                    Button::new("scene.button.loading")
                        .label("Saving")
                        .primary()
                        .loading(true)
                        .on_click(|_, _| {}),
                )
                .child(
                    Button::new("scene.button.selected")
                        .label("Selected")
                        .secondary()
                        .selected(true)
                        .on_click(|_, _| {}),
                ),
        )
        .child(
            row(&theme)
                .child(Button::new("scene.button.xs").label("Extra small").xs())
                .child(Button::new("scene.button.sm").label("Small").small())
                .child(Button::new("scene.button.md").label("Medium").medium())
                .child(Button::new("scene.button.lg").label("Large").large()),
        )
        .into_any_element()
}

/// The search surfaces the scene shows, kept across frames.
///
/// Both hold a [`TextInput`], which owns a caret and a selection that outlive
/// a frame, so they are built once and driven once.
pub(super) struct SceneSearch {
    field: Entity<SearchField>,
    replace: Entity<FindReplace>,
}

impl Global for SceneSearch {}

pub(super) fn ensure_search(window: &mut Window, cx: &mut App) {
    if cx.has_global::<SceneSearch>() {
        return;
    }
    let field = cx.new(|cx| SearchField::new("scene.search.field", window, cx));
    field.update(cx, |field, cx| {
        field.set_query("transport", cx);
        field.set_count(
            HitCount::Known {
                total: 12,
                current: Some(2),
            },
            cx,
        );
    });

    let replace = cx.new(|cx| FindReplace::new("scene.search.replace", window, cx));
    replace.update(cx, |replace, cx| {
        replace.search_field().update(cx, |field, cx| {
            field.set_query("transport", cx);
        });
        replace.replacement_input().update(cx, |input, cx| {
            input.set_value("delivery", cx);
        });
        replace.set_count(
            HitCount::Known {
                total: 12,
                current: Some(2),
            },
            cx,
        );
    });

    cx.set_global(SceneSearch { field, replace });
}

pub(super) fn search_field(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_search(window, cx);
    let field = cx.global::<SceneSearch>().field.clone();
    let theme = cx.theme().clone();

    // The three counts a field must keep apart. No pointer position produces
    // them at once, so each is stated.
    stack(&theme)
        .w(px(620.0))
        .child(field)
        .child(caption(
            &theme,
            "counting is not none, and too many is not a total",
        ))
        .child(
            row(&theme)
                .child(hit_count_sample(&theme, "counting", HitCount::Counting))
                .child(hit_count_sample(&theme, "none", HitCount::None))
                .child(hit_count_sample(
                    &theme,
                    "too many",
                    HitCount::TooMany { counted: 500 },
                )),
        )
        .child(caption(&theme, "the current hit is not the other hits"))
        .child(
            div().child(
                HighlightedText::new(
                    "The transport reports what it did; the transport never decides.",
                )
                .id("scene.search.line")
                .hits([4..13, 39..48])
                .current(1),
            ),
        )
        .into_any_element()
}

/// One hit count, rendered on its own so the states can be seen side by side.
pub(super) fn hit_count_sample(theme: &Theme, label: &'static str, count: HitCount) -> gpui::Div {
    div()
        .column()
        .gap(px(theme.spacing.xs))
        .child(caption(theme, label))
        .child(
            div()
                .px(px(theme.spacing.sm))
                .py(px(theme.spacing.xs))
                .hairline(theme)
                .radius(theme, Radius::Control)
                .child(crate::foundation::text(
                    theme,
                    TypeScale::Label,
                    count.name(),
                )),
        )
}

pub(super) fn find_replace(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_search(window, cx);
    let replace = cx.global::<SceneSearch>().replace.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(620.0))
        // Replace all says how many before it does any, so nobody agrees to a
        // number they were never shown.
        .child(caption(
            &theme,
            "replace all names its count before it acts",
        ))
        .child(replace)
        .into_any_element()
}

pub(super) fn upload_list(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(560.0))
        .child(caption(
            &theme,
            "a refusal is not a failure, and only a failure is offered a retry",
        ))
        .child(
            UploadList::new("scene.uploads")
                .dropzone(
                    Dropzone::new("scene.uploads.zone", "Drop files to attach")
                        .hint("PDF, PNG, or plain text")
                        .on_files(|_, _, _| {}),
                )
                .uploads([
                    Upload::new("brief", "brief.pdf").size("1.2 MB").done(),
                    Upload::new("capture", "capture.png")
                        .size("4.8 MB")
                        .uploading(0.4),
                    Upload::new("notes", "notes.txt").size("12 KB"),
                    Upload::new("archive", "archive.zip")
                        .size("240 MB")
                        .failed("The connection dropped."),
                    Upload::new("installer", "installer.exe")
                        .size("64 MB")
                        .refused("This zone does not take programs."),
                ])
                .on_retry(|_, _, _| {})
                .on_cancel(|_, _, _| {})
                .on_remove(|_, _, _| {}),
        )
        .into_any_element()
}

/// The cascader owns only the open surface and path, so its scene keeps one
/// view alive across capture frames.
pub(super) struct SceneCascader {
    cascader: Entity<Cascader>,
}

impl Global for SceneCascader {}

pub(super) fn cascader(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneCascader>() {
        let cascader = cx.new(|cx| {
            Cascader::new("scene.cascader", window, cx)
                .name("Fixture destination")
                .selected("release-notes")
                .options([
                    CascaderOption::new("guides", "Guides").children([
                        CascaderOption::new("getting-started", "Getting started"),
                        CascaderOption::new("configuration", "Configuration"),
                    ]),
                    CascaderOption::new("reference", "Reference").loading_children(),
                    CascaderOption::new("archive", "Archive").unavailable_children(
                        "The fixture host does not provide archived sections.",
                    ),
                    CascaderOption::new("release-notes", "Release notes"),
                    CascaderOption::new("managed", "Managed section").disabled(true),
                ])
        });
        cascader.update(cx, |cascader, cx| cascader.open(window, cx));
        cx.set_global(SceneCascader { cascader });
    }
    let theme = cx.theme().clone();
    let cascader = cx.global::<SceneCascader>().cascader.clone();
    stack(&theme)
        .w(px(680.0))
        .child(caption(
            &theme,
            "caller-owned hierarchy and value; the open path belongs only to the view",
        ))
        // The trigger is given the width its own popup has, so the scene does
        // not show a control that disagrees with the surface it opens.
        .child(div().w(px(380.0)).child(cascader))
        .into_any_element()
}

pub(super) fn choice(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    div()
        .flex()
        .flex_col()
        .gap(px(theme.space(Space::Md)))
        .p(px(theme.space(Space::Lg)))
        .w(px(360.0))
        .child(
            Checkbox::new("scene.choice.telemetry")
                .label("Send anonymous usage data")
                .description("Counts only, never file contents")
                .checked(true)
                .on_change(|_, _, _| {}),
        )
        .child(
            Checkbox::new("scene.choice.partial")
                .label("Some providers enabled")
                .mixed()
                .on_change(|_, _, _| {}),
        )
        .child(
            Checkbox::new("scene.choice.locked")
                .label("Managed by policy")
                .checked(true)
                .disabled(true),
        )
        .child(
            Radio::new("scene.choice.ask")
                .label("Ask before every action")
                .selected(true)
                .on_select(|_, _| {}),
        )
        .child(
            Radio::new("scene.choice.auto")
                .label("Run without asking")
                .description("Consequential actions still require approval")
                .on_select(|_, _| {}),
        )
        .child(
            Switch::new("scene.choice.preview")
                .label("Preview releases")
                .on(true)
                .on_change(|_, _, _| {}),
        )
        .child(
            Slider::new("scene.choice.temperature")
                .label("Temperature")
                .range(0.0, 2.0)
                .step(0.1)
                .value(0.7)
                .display("0.7")
                .on_change(|_, _, _| {}),
        )
        .child(
            Slider::new("scene.choice.window")
                .label("Window")
                .range(0.0, 1.0)
                .values(0.2, 0.8)
                .marks([0.0, 0.25, 0.5, 0.75, 1.0])
                .display("0.2 – 0.8")
                .on_range_change(|_, _, _, _| {}),
        )
        .into_any_element()
}

/// The form scene's controls, kept across frames.
///
/// Every one of these owns editing state — a caret, a query, an open list —
/// so they are built once. Building them once is also what makes the capture
/// static.
pub(super) struct SceneForm {
    name: Entity<TextInput>,
    retention: Entity<NumberInput>,
    region: Entity<Combobox>,
    labels: Entity<TagInput>,
}

impl Global for SceneForm {}

pub(super) fn ensure_form(window: &mut Window, cx: &mut App) {
    if cx.has_global::<SceneForm>() {
        return;
    }
    let name = cx.new(|cx| {
        TextInput::new("scene.form.name", window, cx)
            .text("Runs 2024")
            .required(true)
            .invalid(true)
    });
    let retention = cx.new(|cx| {
        // The host holds ninety days while its own limit is sixty. The field
        // shows the number that is actually set and says it is out of range,
        // rather than quietly drawing a number nobody chose.
        NumberInput::new("scene.form.retention", window, cx)
            .value(90.0)
            .range(1.0, 60.0)
            .step(5.0)
            .prefix("~")
            .unit("days")
            .required(true)
    });
    let region = cx.new(|cx| {
        let mut options = (0..14)
            .map(|index| {
                SelectOption::new(
                    format!("model-{index:02}"),
                    format!("Agent model {index:02}"),
                )
            })
            .collect::<Vec<_>>();
        options.push(SelectOption::new("unknown", "Unknown model").description(
            "This model may not support chat in direct mode.\nChoose a chat-capable model.",
        ));
        options.push(SelectOption::new("managed", "Managed model").disabled(true));
        Combobox::new("scene.form.region", window, cx)
            .name("All Agent models")
            .options(options)
            .selected("unknown")
            .placeholder("Choose an Agent model")
    });
    let labels = cx.new(|cx| {
        TagInput::new("scene.form.labels", window, cx)
            .tags(["indexing", "nightly", "verified"])
            .placeholder("Add a label")
            .max(5)
            .reorderable(true)
            .collapse_at(5)
    });
    region.update(cx, |combobox, cx| {
        combobox.set_query("Unknown model", cx);
    });
    cx.set_global(SceneForm {
        name,
        retention,
        region,
        labels,
    });
}

pub(super) fn form(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_form(window, cx);
    let form = cx.global::<SceneForm>();
    let (name, retention, region, labels) = (
        form.name.clone(),
        form.retention.clone(),
        form.region.clone(),
        form.labels.clone(),
    );
    let theme = cx.theme().clone();

    stack(&theme)
        .w(px(420.0))
        .h(px(920.0))
        .child(
            FormField::new("scene.form.name.form-field", "Workspace name")
                .control("scene.form.name")
                .required(true)
                // The description says what the field is for and the error
                // says what went wrong; neither answers for the other.
                .description("Shown wherever this workspace appears.")
                .error("A workspace with this name already exists.")
                .child(name),
        )
        .child(
            FormField::new("scene.form.retention.form-field", "Retention")
                .control("scene.form.retention")
                .required(true)
                .description("How long a finished run is kept.")
                .error("This workspace allows at most 60 days.")
                .child(retention),
        )
        .child(
            FormField::new("scene.form.visibility.form-field", "Visibility")
                .control("scene.form.visibility")
                .description("Who can open the runs in this workspace.")
                .child(
                    SegmentedControl::new("scene.form.visibility")
                        .label("Visibility")
                        .segments([
                            Segment::new("private", "Private"),
                            Segment::new("team", "Team"),
                            Segment::new("public", "Public").disabled(true),
                        ])
                        .selected("team")
                        .on_select(|_, _, _| {}),
                ),
        )
        .child(
            FormField::new("scene.form.labels.form-field", "Labels")
                .control("scene.form.labels")
                // The keystroke lives in the hint, so the description does not
                // spend a second line repeating it.
                .description("At most five, and each one only once.")
                .hint("enter")
                .child(labels),
        )
        .child(
            div().mt_auto().child(
                FormField::new("scene.form.region.form-field", "All Agent models")
                    .control("scene.form.region")
                    .description("Choose a chat-capable model for direct runs.")
                    .child(region),
            ),
        )
        .into_any_element()
}

/// The password in the canonical sign-in composition, kept across frames.
pub(super) struct SceneAuthSignIn {
    password: Entity<PasswordInput>,
}

impl Global for SceneAuthSignIn {}

pub(super) fn ensure_auth_sign_in(window: &mut Window, cx: &mut App) {
    if cx.has_global::<SceneAuthSignIn>() {
        return;
    }
    let password = cx.new(|cx| {
        PasswordInput::new("scene.auth.sign-in.password", window, cx)
            .name("Password")
            .placeholder("Enter password")
            .required(true)
    });
    cx.set_global(SceneAuthSignIn { password });
}

pub(super) fn auth_sign_in(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_auth_sign_in(window, cx);
    let password = cx.global::<SceneAuthSignIn>().password.clone();
    let theme = cx.theme().clone();
    let card_id = "scene.auth.sign-in.card";
    let title = crate::foundation::text(&theme, TypeScale::Title, "Sign in").semantic_in(
        cx,
        NodeSpec::new("scene.auth.sign-in.title", Role::Text)
            .parent(card_id)
            .text("Sign in"),
    );

    stack(&theme)
        .w(px(440.0))
        .child(
            Card::new().id(card_id).padded(true).child(
                div()
                    .column()
                    .gap_token(&theme, Space::Md)
                    .child(title)
                    .child(
                        Callout::new("Credentials are verified by the caller.", Tone::Neutral)
                            .id("scene.auth.sign-in.boundary"),
                    )
                    .child(
                        FormField::new("scene.auth.sign-in.password.field", "Password")
                            .control("scene.auth.sign-in.password")
                            .required(true)
                            .child(password),
                    )
                    .child(
                        Button::new("scene.auth.sign-in.submit")
                            .label("Sign in")
                            .full_width(true)
                            .on_click(|_, _| {}),
                    )
                    .child(
                        Button::new("scene.auth.sign-in.passkey")
                            .label("Continue with passkey")
                            .icon(Icon::Key)
                            .secondary()
                            .full_width(true)
                            .on_click(|_, _| {}),
                    )
                    .child(
                        Button::new("scene.auth.sign-in.organization")
                            .label("Continue with organization sign-on")
                            .icon(Icon::Global)
                            .secondary()
                            .full_width(true)
                            .on_click(|_, _| {}),
                    )
                    .child(
                        Button::new("scene.auth.sign-in.recovery")
                            .label("Use a recovery option")
                            .link()
                            .on_click(|_, _| {}),
                    ),
            ),
        )
        .into_any_element()
}

/// The one logical code input in the canonical verification composition.
pub(super) struct SceneAuthVerification {
    code: Entity<OneTimeCodeInput>,
}

impl Global for SceneAuthVerification {}

pub(super) fn ensure_auth_verification(window: &mut Window, cx: &mut App) {
    if cx.has_global::<SceneAuthVerification>() {
        return;
    }
    let code = cx.new(|cx| {
        OneTimeCodeInput::new("scene.auth.verification.code", window, cx)
            .name("Verification code")
            .slots(6)
            .required(true)
    });
    cx.set_global(SceneAuthVerification { code });
}

pub(super) fn auth_verification(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_auth_verification(window, cx);
    let code = cx.global::<SceneAuthVerification>().code.clone();
    let theme = cx.theme().clone();
    let card_id = "scene.auth.verification.card";
    let title = crate::foundation::text(&theme, TypeScale::Title, "Verify sign-in").semantic_in(
        cx,
        NodeSpec::new("scene.auth.verification.title", Role::Text)
            .parent(card_id)
            .text("Verify sign-in"),
    );

    stack(&theme)
        .w(px(440.0))
        .child(
            Card::new().id(card_id).padded(true).child(
                div()
                    .column()
                    .gap_token(&theme, Space::Md)
                    .child(title)
                    .child(
                        Callout::new(
                            "Enter the code from your authenticator or recovery method.",
                            Tone::Neutral,
                        )
                        .id("scene.auth.verification.guidance"),
                    )
                    .child(
                        FormField::new("scene.auth.verification.code.field", "Verification code")
                            .control("scene.auth.verification.code")
                            .required(true)
                            .child(code),
                    )
                    .child(
                        Button::new("scene.auth.verification.submit")
                            .label("Verify")
                            .full_width(true)
                            .on_click(|_, _| {}),
                    )
                    .child(
                        Button::new("scene.auth.verification.alternative")
                            .label("Use another method")
                            .secondary()
                            .full_width(true)
                            .on_click(|_, _| {}),
                    )
                    .child(
                        Button::new("scene.auth.verification.recovery")
                            .label("Use a recovery option")
                            .link()
                            .on_click(|_, _| {}),
                    ),
            ),
        )
        .into_any_element()
}

/// The split button the actions scene shows, kept across frames.
pub(super) struct SceneActions {
    split: Entity<SplitButton>,
}

impl Global for SceneActions {}

pub(super) fn ensure_actions(window: &mut Window, cx: &mut App) {
    if cx.has_global::<SceneActions>() {
        return;
    }
    let split = cx.new(|cx| {
        SplitButton::new("scene.actions.publish", window, cx)
            .label("Publish")
            .primary()
            .on_click(|_, _| {})
            .items(
                [
                    MenuItem::command("publish.draft", "Save as draft"),
                    MenuItem::command("publish.schedule", "Schedule…").shortcut("cmd-shift-s"),
                    MenuItem::separator("publish.rule"),
                    MenuItem::command("publish.export", "Export without publishing"),
                ],
                cx,
            )
    });
    split.update(cx, |split, cx| split.open_menu(window, cx));
    cx.set_global(SceneActions { split });
}

pub(super) fn actions(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_actions(window, cx);
    let split = cx.global::<SceneActions>().split.clone();
    let theme = cx.theme().clone();

    stack(&theme)
        .w(px(560.0))
        .h(px(360.0))
        .child(
            row(&theme)
                .child(
                    IconButton::new("scene.actions.copy", Icon::Copy, "Copy run id")
                        .on_click(|_, _| {}),
                )
                .child(
                    IconButton::new("scene.actions.rename", Icon::Pen, "Rename run")
                        .secondary()
                        .on_click(|_, _| {}),
                )
                .child(
                    IconButton::new("scene.actions.refresh", Icon::Refresh, "Refresh")
                        .secondary()
                        .loading(true)
                        .on_click(|_, _| {}),
                )
                .child(
                    IconButton::new("scene.actions.delete", Icon::Trash, "Delete run")
                        .danger()
                        .on_click(|_, _| {}),
                )
                .child(
                    IconButton::new("scene.actions.archive", Icon::Archive, "Archive run")
                        .secondary()
                        .disabled(true)
                        .on_click(|_, _| {}),
                ),
        )
        .child(
            row(&theme).child(
                ButtonGroup::new("scene.actions.range")
                    .children([
                        Button::new("scene.actions.range.day")
                            .label("Day")
                            .secondary()
                            .on_click(|_, _| {}),
                        Button::new("scene.actions.range.week")
                            .label("Week")
                            .secondary()
                            .selected(true)
                            .on_click(|_, _| {}),
                        Button::new("scene.actions.range.month")
                            .label("Month")
                            .secondary()
                            .on_click(|_, _| {}),
                    ])
                    .small(),
            ),
        )
        .child(row(&theme).child(split))
        .into_any_element()
}

/// The inputs the scene shows, kept across frames.
///
/// An editable control carries state, so the scene builds its entities once
/// rather than on every frame, which would discard whatever was typed.
pub(super) struct SceneInputs {
    token: Entity<TextInput>,
    disabled: Entity<TextInput>,
    invalid: Entity<TextInput>,
    provider: Entity<Select>,
    notes: Entity<TextArea>,
    review: Entity<TextArea>,
    frozen: Entity<TextArea>,
    message: Entity<TextArea>,
}

impl Global for SceneInputs {}

pub(super) fn ensure_inputs(window: &mut Window, cx: &mut App) {
    if !cx.has_global::<SceneInputs>() {
        let inputs = SceneInputs {
            token: cx.new(|cx| {
                TextInput::new("scene.input.token", window, cx)
                    .name("API token")
                    .placeholder("sk-...")
                    .secret(true)
            }),
            disabled: cx.new(|cx| {
                TextInput::new("scene.input.disabled", window, cx)
                    .name("Disabled")
                    .text("read only")
                    .disabled(true)
            }),
            invalid: cx.new(|cx| {
                TextInput::new("scene.input.invalid", window, cx)
                    .name("Email")
                    .text("not an email")
                    .invalid(true)
                    .required(true)
            }),
            provider: cx.new(|cx| {
                Select::new("scene.input.provider", window, cx)
                    .name("Provider")
                    .options([
                        SelectOption::new("anthropic", "Anthropic").group("Hosted"),
                        SelectOption::new("openai", "OpenAI")
                            .description("Requires a key")
                            .group("Hosted"),
                        SelectOption::new("local", "Local runtime")
                            .disabled(true)
                            .group("On this machine"),
                    ])
                    .selected("anthropic")
                    .clearable(true)
                    .placeholder("Choose a provider")
            }),
            notes: cx.new(|cx| {
                TextArea::new("scene.textarea.notes", window, cx)
                    .text(
                        "The refusal is shown exactly as the host worded it, and the last \
                         verified value stays on screen.",
                    )
                    .rows(3)
                    .max_rows(6)
                    .max_length(240)
            }),
            review: cx.new(|cx| {
                TextArea::new("scene.textarea.review", window, cx)
                    .placeholder("What changed, and why")
                    .rows(3)
            }),
            frozen: cx.new(|cx| {
                TextArea::new("scene.textarea.frozen", window, cx)
                    .text("Set by the administrator.\nThis machine cannot change it.")
                    .rows(2)
                    .disabled(true)
            }),
            message: cx.new(|cx| {
                TextArea::new("scene.textarea.message", window, cx)
                    .placeholder("Ask anything. Enter sends, shift-enter opens a line.")
                    .enter(Enter::Submits)
                    .rows(2)
                    .max_rows(8)
            }),
        };
        // A caret only paints where the keyboard is, so one area takes it:
        // otherwise a capture cannot show a caret at all.
        window.focus(&inputs.review.read(cx).focus_handle(cx), cx);
        cx.set_global(inputs);
    }
}

pub(super) fn input(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_inputs(window, cx);
    let inputs = cx.global::<SceneInputs>();
    let (token, disabled, invalid, provider) = (
        inputs.token.clone(),
        inputs.disabled.clone(),
        inputs.invalid.clone(),
        inputs.provider.clone(),
    );
    let theme = cx.theme().clone();

    div()
        .flex()
        .flex_col()
        .gap(px(theme.space(Space::Md)))
        .p(px(theme.space(Space::Lg)))
        .w(px(360.0))
        .child(token)
        .child(disabled)
        .child(invalid)
        .child(provider)
        .into_any_element()
}

pub(super) fn textarea(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_inputs(window, cx);
    let inputs = cx.global::<SceneInputs>();
    let (notes, review, frozen, message) = (
        inputs.notes.clone(),
        inputs.review.clone(),
        inputs.frozen.clone(),
        inputs.message.clone(),
    );
    let theme = cx.theme().clone();

    div()
        .flex()
        .flex_col()
        .gap(px(theme.space(Space::Md)))
        .p(px(theme.space(Space::Lg)))
        .w(px(360.0))
        .child(notes)
        .child(review)
        .child(frozen)
        // The other enter policy, for text that is a message rather than a
        // value. Nothing about it looks different, which is the point: what
        // changes is which key is the common act.
        .child(message)
        .into_any_element()
}

pub(super) fn dropzone(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    // No single pointer position can produce all three states at once, so each
    // zone is pinned to the one it is here to show.
    stack(&theme)
        .w(px(560.0))
        .child(caption(&theme, "idle, accepting, refusing"))
        .child(
            row(&theme)
                .items_stretch()
                .child(
                    div().flex_1().child(
                        Dropzone::new("scene.dropzone.idle", "Drop files to attach")
                            .hint("PDF, PNG, or plain text")
                            .state(DropzoneState::Idle)
                            .on_files(|_, _, _| {}),
                    ),
                )
                .child(
                    div().flex_1().child(
                        Dropzone::new("scene.dropzone.accepting", "Drop files to attach")
                            .hint("PDF, PNG, or plain text")
                            .state(DropzoneState::Accepting)
                            .on_files(|_, _, _| {}),
                    ),
                )
                .child(
                    div().flex_1().child(
                        Dropzone::new("scene.dropzone.refusing", "Drop files to attach")
                            .refusal("A folder cannot be attached.")
                            .state(DropzoneState::Refusing)
                            .on_files(|_, _, _| {}),
                    ),
                ),
        )
        .into_any_element()
}

pub(super) fn settings(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(620.0))
        .child(
            SettingsSection::new("scene.settings.general", "General")
                .description("How this workspace behaves")
                .row(
                    SettingsRow::new("scene.settings.general.autosave", "Save automatically")
                        .description("Write changes as they happen")
                        .control(
                            Switch::new("scene.settings.general.autosave.switch")
                                .named("Save automatically")
                                .on(true)
                                .on_change(|_, _, _| {}),
                        ),
                )
                .row(
                    SettingsRow::new("scene.settings.general.runtime", "Native runtime")
                        .description("Runs work on this machine instead of a host")
                        .badge("Requires restart")
                        .control(
                            Switch::new("scene.settings.general.runtime.switch")
                                .named("Native runtime")
                                .on(false)
                                .on_change(|_, _, _| {}),
                        ),
                )
                .row(
                    SettingsRow::new("scene.settings.general.telemetry", "Usage reporting")
                        .description("Nobody on this machine can change this")
                        .value("Off")
                        .managed("your administrator"),
                ),
        )
        .child(
            SettingsSection::new("scene.settings.sync", "Synchronisation")
                .description("What travels between machines")
                .dimmed_by("This workspace is local, so nothing synchronises.")
                .row(
                    SettingsRow::new("scene.settings.sync.settings", "Sync settings")
                        .value("Off")
                        .control(
                            Switch::new("scene.settings.sync.settings.switch")
                                .named("Sync settings")
                                .on(false)
                                .on_change(|_, _, _| {}),
                        ),
                )
                .row(
                    SettingsRow::new("scene.settings.sync.history", "Sync history")
                        .value("Off")
                        .control(
                            Switch::new("scene.settings.sync.history.switch")
                                .named("Sync history")
                                .on(false)
                                .on_change(|_, _, _| {}),
                        ),
                ),
        )
        .into_any_element()
}

pub(super) fn filter_bar(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(720.0))
        .child(
            FilterBar::new("scene.filter-bar.runs")
                .conditions([
                    FilterCondition::new("status", "Status", "is", "failed"),
                    FilterCondition::new("owner", "Owner", "is", "fixture-owner"),
                    FilterCondition::new("started", "Started", "after", "09:00"),
                ])
                .count(ResultCount::Known(14))
                .noun("runs")
                .on_add(|_, _| {})
                .on_remove(|_, _, _| {})
                .on_clear(|_, _| {}),
        )
        .child(caption(&theme, "counting is not zero"))
        .child(
            FilterBar::new("scene.filter-bar.counting")
                .conditions([FilterCondition::new("status", "Status", "is", "queued")])
                .count(ResultCount::Counting)
                .on_add(|_, _| {})
                .on_remove(|_, _, _| {})
                .on_clear(|_, _| {}),
        )
        .into_any_element()
}

pub(super) fn inline_edit(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(420.0))
        .child(caption(
            &theme,
            "reading, editing, and a save that did not take",
        ))
        .child(
            InlineEdit::new("scene.inline-edit.title", "Indexing the workspace")
                .on_edit(|_, _| {})
                .on_commit(|_, _, _| {})
                .on_cancel(|_, _| {}),
        )
        .child(
            InlineEdit::new("scene.inline-edit.owner", "fixture-owner")
                .editing(true)
                .on_edit(|_, _| {})
                .on_commit(|_, _, _| {})
                .on_cancel(|_, _| {}),
        )
        .child(
            InlineEdit::new(
                "scene.inline-edit.note",
                "Retry after the host is reachable",
            )
            .editing(true)
            .failure("The host refused this change. What you typed is still here.")
            .on_edit(|_, _| {})
            .on_commit(|_, _, _| {})
            .on_cancel(|_, _| {}),
        )
        .child(
            InlineEdit::new("scene.inline-edit.policy", "Set by the administrator")
                .disabled(true)
                .on_edit(|_, _| {}),
        )
        .into_any_element()
}

#[derive(Clone)]
pub(super) struct SceneRecorders {
    idle: Entity<KeybindingRecorder>,
    recording: Entity<KeybindingRecorder>,
    captured: Entity<KeybindingRecorder>,
    conflicting: Entity<KeybindingRecorder>,
}

impl Global for SceneRecorders {}

pub(super) fn keybinding(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneRecorders>() {
        let idle = cx.new(|cx| {
            KeybindingRecorder::new("scene.keybinding.idle", window, cx).label("Open workspace")
        });
        let recording = cx.new(|cx| {
            KeybindingRecorder::new("scene.keybinding.recording", window, cx)
                .label("Command palette")
        });
        let captured = cx.new(|cx| {
            KeybindingRecorder::new("scene.keybinding.captured", window, cx)
                .label("Toggle terminal")
                .binding("ctrl-`")
        });
        let conflicting = cx.new(|cx| {
            KeybindingRecorder::new("scene.keybinding.conflicting", window, cx)
                .label("Split editor")
                .binding("cmd-shift-p")
                // The host's words, not the recorder's: it has no keymap.
                .conflict(Some("Already opens the command palette"))
        });
        // Recording is a state, not a gesture, so the scene puts one recorder
        // into it by hand rather than waiting for a keystroke that a still
        // image could not photograph anyway.
        recording.update(cx, |recorder, cx| recorder.start(window, cx));
        cx.set_global(SceneRecorders {
            idle,
            recording,
            captured,
            conflicting,
        });
    }
    let recorders = cx.global::<SceneRecorders>().clone();
    let theme = cx.theme().clone();

    // A recorder carries its name in the tree rather than drawing one, the way
    // every other control here does, so the scene puts it where a keymap page
    // would: in a settings row that states what the binding is for.
    stack(&theme)
        .w(px(680.0))
        .child(
            SettingsSection::new("scene.keybinding.keymap", "Keyboard shortcuts")
                .description("Recording captures the next keystroke instead of acting on it.")
                .row(
                    SettingsRow::new("scene.keybinding.row.open", "Open workspace")
                        .description("Nothing is bound yet")
                        .control(recorders.idle),
                )
                .row(
                    SettingsRow::new("scene.keybinding.row.palette", "Command palette")
                        .description("Listening for a keystroke")
                        .control(recorders.recording),
                )
                .row(
                    SettingsRow::new("scene.keybinding.row.terminal", "Toggle terminal")
                        .control(recorders.captured),
                )
                .row(
                    SettingsRow::new("scene.keybinding.row.split", "Split editor")
                        .description("The host judged this one, and said so")
                        .control(recorders.conflicting),
                ),
        )
        .child(caption(
            &theme,
            "Escape ends recording without capturing, so escape cannot be bound \
             unless the caller turns allow_escape on.",
        ))
        .into_any_element()
}

#[derive(Clone)]
pub(super) struct SceneKeymapEditor(Entity<KeymapEditor>);

impl Global for SceneKeymapEditor {}

pub(super) fn keymap_editor(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneKeymapEditor>() {
        let editor = cx.new(|cx| {
            KeymapEditor::new("scene.keymap-editor", window, cx).commands([
                KeymapCommand::new("workspace.open", "Open workspace")
                    .context("Workspace")
                    .defaults(["cmd-o"])
                    .bindings([
                        KeymapBinding::new("user", "cmd-shift-o")
                            .conflict("Already opens recent workspaces")
                            .provenance("User keymap"),
                        KeymapBinding::new("workspace", "ctrl-o").provenance("Workspace keymap"),
                    ])
                    .searchable("Open a folder or project", ["folder", "project"]),
                KeymapCommand::new("terminal.toggle", "Toggle terminal")
                    .context("Terminal")
                    .defaults(["ctrl-`"])
                    .bindings([KeymapBinding::new("default", "ctrl-`")])
                    .searchable("Show the integrated terminal", ["panel", "console"]),
                KeymapCommand::new("policy.locked", "Managed shortcut")
                    .context("Workspace")
                    .defaults(["cmd-l"])
                    .bindings([KeymapBinding::new("managed", "cmd-l").provenance("Host policy")])
                    .refused("This binding is managed by the host."),
            ])
        });
        cx.set_global(SceneKeymapEditor(editor));
    }
    let editor = cx.global::<SceneKeymapEditor>().0.clone();
    let theme = cx.theme().clone();

    stack(&theme)
        .w(px(760.0))
        .child(editor)
        .child(caption(
            &theme,
            "Bindings remain caller-owned; add, remove, and reset are intents.",
        ))
        .into_any_element()
}

pub(super) fn toggle(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .child(caption(&theme, "A button that stays in"))
        .child(
            row(&theme)
                .child(
                    Toggle::new("scene.toggle.bold")
                        .label("Bold")
                        .pressed(true)
                        .on_press(|_, _, _| {}),
                )
                .child(
                    Toggle::new("scene.toggle.italic")
                        .label("Italic")
                        .on_press(|_, _, _| {}),
                )
                .child(
                    Toggle::new("scene.toggle.review")
                        .label("Review mode")
                        .secondary()
                        .pressed(true)
                        .on_press(|_, _, _| {}),
                )
                .child(
                    Toggle::new("scene.toggle.locked")
                        .label("Locked")
                        .disabled(true),
                ),
        )
        .child(caption(&theme, "Any number in at once"))
        .child(
            ToggleGroup::new("scene.toggle-group.format")
                .label("Formatting")
                .selection(ToggleSelection::Any)
                .items([
                    ToggleItem::new("bold", "Bold"),
                    ToggleItem::new("italic", "Italic"),
                    ToggleItem::new("underline", "Underline").disabled(true),
                ])
                .pressed_ids(&["bold", "italic"])
                .on_change(|_, _, _, _| {}),
        )
        .child(caption(
            &theme,
            "One or none, which a segmented strip cannot say",
        ))
        .child(
            ToggleGroup::new("scene.toggle-group.density")
                .label("Density")
                .selection(ToggleSelection::AtMostOne)
                .items([
                    ToggleItem::new("compact", "Compact"),
                    ToggleItem::new("cosy", "Cosy"),
                    ToggleItem::new("roomy", "Roomy"),
                ])
                .pressed_ids(&["cosy"])
                .on_change(|_, _, _, _| {}),
        )
        .into_any_element()
}

pub(super) fn copy_button(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_ordinary(window, cx);
    let scene = cx.global::<SceneOrdinary>();
    let idle = scene.copy_idle.clone();
    let copied = scene.copy.clone();
    let refused = scene.copy_refused.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .child(caption(&theme, "Nobody has pressed it yet"))
        .child(idle)
        .child(caption(&theme, "The clipboard took it"))
        .child(copied)
        .child(caption(&theme, "It did not go through, and says so"))
        .child(refused)
        .into_any_element()
}

pub(super) fn color_picker(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let current = theme.colors.info;
    stack(&theme)
        .child(caption(
            &theme,
            "the value is caller-owned; presets and recents are host lists",
        ))
        .child(
            ColorPicker::new("scene.color.picker", current)
                .alpha(true)
                .presets([
                    theme.colors.danger,
                    theme.colors.warning,
                    theme.colors.info,
                    theme.colors.success,
                    theme.colors.accent,
                ])
                .recent([theme.colors.accent, current])
                .on_change(|_, _, _| {}),
        )
        .child(caption(&theme, "a swatch reports the colour it was given"))
        .child(
            row(&theme)
                .child(ColorSwatch::new(
                    "scene.color.swatch.accent",
                    theme.colors.accent,
                ))
                .child(
                    ColorSwatch::new("scene.color.swatch.selected", current)
                        .selected(true)
                        .on_click(|_, _, _| {}),
                )
                .child(
                    ColorSwatch::new("scene.color.swatch.disabled", theme.colors.danger)
                        .disabled(true),
                ),
        )
        .into_any_element()
}
