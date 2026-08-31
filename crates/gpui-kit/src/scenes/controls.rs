//! Things a pointer or a keyboard acts on directly.

use super::support::*;

pub(super) fn button(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .child(caption(&theme, "how much weight the action carries"))
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
        .child(caption(
            &theme,
            "refused is not in flight, and neither is the current answer",
        ))
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
        .child(caption(&theme, "the control scale, smallest to largest"))
        .child(
            row(&theme)
                .child(Button::new("scene.button.xs").label("Extra small").xs())
                .child(Button::new("scene.button.sm").label("Small").small())
                .child(Button::new("scene.button.md").label("Medium").medium())
                .child(Button::new("scene.button.lg").label("Large").large()),
        )
        .child(caption(
            &theme,
            "the shared tiers, resolved against a palette colour",
        ))
        .children(["indigo", "teal", "red"].map(|group| {
            row(&theme).children(
                [
                    Variant::Filled,
                    Variant::Light,
                    Variant::Subtle,
                    Variant::Default,
                    Variant::Transparent,
                ]
                .map(|tier| {
                    Button::new(format!("scene.button.{group}.{}", tier.name()))
                        .label(tier.name())
                        .variant(tier)
                        .color(SharedString::from(group))
                        .on_click(|_, _| {})
                }),
            )
        }))
        .child(caption(
            &theme,
            "the same tiers, chosen: selection is a stronger step of the ladder the \
             caller picked, so the colour survives being the current answer",
        ))
        .child(
            row(&theme).children(
                [
                    Variant::Filled,
                    Variant::Light,
                    Variant::Subtle,
                    Variant::Default,
                    Variant::Transparent,
                ]
                .map(|tier| {
                    Button::new(format!("scene.button.chosen.{}", tier.name()))
                        .label(tier.name())
                        .variant(tier)
                        .color(SharedString::from("indigo"))
                        .selected(true)
                        .on_click(|_, _| {})
                }),
            ),
        )
        .into_any_element()
}

/// The search surfaces the scene shows, kept across frames.
///
/// Both hold a [`TextInput`], which owns a caret and a selection that outlive
/// a frame, so they are built once and driven once.
pub(super) struct SceneSearch {
    field: Entity<SearchField>,
    counting: Entity<SearchField>,
    none: Entity<SearchField>,
    too_many: Entity<SearchField>,
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

    // The three counts a field must keep apart are shown by three fields.
    // Rendering their published names as chips said what the tree calls them,
    // which is not what a reader of the field would ever see.
    let mut sample = |id: &'static str, query: &'static str, count: HitCount, cx: &mut App| {
        let field = cx.new(|cx| SearchField::new(id, window, cx));
        field.update(cx, |field, cx| {
            field.set_query(query, cx);
            field.set_count(count, cx);
        });
        field
    };
    let counting = sample("scene.search.counting", "transport", HitCount::Counting, cx);
    let none = sample("scene.search.none", "teleport", HitCount::None, cx);
    let too_many = sample(
        "scene.search.too-many",
        "e",
        HitCount::TooMany { counted: 500 },
        cx,
    );

    let replace = cx.new(|cx| FindReplace::new("scene.search.replace", window, cx));
    replace.update(cx, |replace, cx| {
        replace.search_field().update(cx, |field, cx| {
            field.set_query("transport", cx);
            // Case and whole-word are the host's state, and a find surface
            // that cannot show them is not the one a product ships.
            field.set_match_case(Some(true), cx);
            field.set_whole_word(Some(false), cx);
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

    cx.set_global(SceneSearch {
        field,
        counting,
        none,
        too_many,
        replace,
    });
}

pub(super) fn search_field(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_search(window, cx);
    let field = cx.global::<SceneSearch>().field.clone();
    let theme = cx.theme().clone();

    let counting = cx.global::<SceneSearch>().counting.clone();
    let none = cx.global::<SceneSearch>().none.clone();
    let too_many = cx.global::<SceneSearch>().too_many.clone();

    // The three counts a field must keep apart. No pointer position produces
    // them at once, so each is stated.
    stack(&theme)
        .w(px(620.0))
        .child(field)
        .child(caption(
            &theme,
            "counting is not none, and too many is not a total",
        ))
        .child(counting)
        .child(none)
        .child(too_many)
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
        .child(caption(
            &theme,
            "Vertical range uses the same value, marks, and keyboard contract",
        ))
        .child(
            div().h(px(220.0)).child(
                Slider::new("scene.choice.vertical")
                    .label("Vertical")
                    .orientation(SliderOrientation::Vertical)
                    .range(0.0, 100.0)
                    .step(10.0)
                    .value(60.0)
                    .marks([0.0, 25.0, 50.0, 75.0, 100.0])
                    .display("60")
                    .on_change(|_, _, _| {}),
            ),
        )
        .into_any_element()
}

/// The searchable multi-value control is kept alive so its query and open
/// state survive gallery rebuilds.
pub(super) struct SceneMultiSelect {
    control: Entity<MultiSelect>,
}

impl Global for SceneMultiSelect {}

pub(super) fn multi_select(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneMultiSelect>() {
        let control = cx.new(|cx| {
            MultiSelect::new("scene.multi-select", window, cx)
                .name("Enabled providers")
                .placeholder("Choose providers")
                .selected(["native", "remote"])
                .options([
                    SelectOption::new("native", "Native runtime")
                        .description("Runs on this machine"),
                    SelectOption::new("remote", "Remote gateway")
                        .description("Uses the workspace gateway"),
                    SelectOption::new("preview", "Preview models").disabled(true),
                    SelectOption::new("archive", "Archive models"),
                ])
                .clearable(true)
        });
        control.update(cx, |control, cx| control.open(window, cx));
        cx.set_global(SceneMultiSelect { control });
    }
    let theme = cx.theme().clone();
    let control = cx.global::<SceneMultiSelect>().control.clone();
    stack(&theme)
        .w(px(520.0))
        .child(caption(
            &theme,
            "Selected ids stay with the host; search, chips, and option focus stay with the view",
        ))
        .child(control)
        .into_any_element()
}

/// The two-pane assignment control demonstrates source/target selection
/// without allowing the component to mutate either collection.
pub(super) struct SceneTransferList {
    control: Entity<TransferList>,
}

impl Global for SceneTransferList {}

pub(super) fn transfer_list(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneTransferList>() {
        let control = cx.new(|cx| {
            TransferList::new("scene.transfer-list", window, cx)
                .source([
                    TransferItem::new("runtime", "Native runtime"),
                    TransferItem::new("gateway", "Remote gateway"),
                    TransferItem::new("preview", "Preview models").disabled(true),
                ])
                .target([TransferItem::new("logs", "Run logs")])
                .source_selected(["gateway"])
                .target_selected(["logs"])
                .source_label("Available capabilities")
                .target_label("Assigned capabilities")
        });
        cx.set_global(SceneTransferList { control });
    }
    let theme = cx.theme().clone();
    let control = cx.global::<SceneTransferList>().control.clone();
    stack(&theme)
        .w(px(720.0))
        .child(caption(
            &theme,
            "Each pane reports stable item intents; the host performs the assignment",
        ))
        .child(control)
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

    // No fixed height and nothing pushed to the bottom: a form is a stack of
    // fields one gap apart, and the 250px of nothing that a pinned last field
    // opened up is not a rhythm anything else in the library keeps.
    stack(&theme)
        .w(px(420.0))
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
                .validation(ValidationState::Validating)
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
            // Beside the strip above, which is on the accent: the answer here
            // is which colour-identified thing, so the segment that holds
            // wears that thing's own colour and the rest of the strip is
            // unchanged.
            FormField::new("scene.form.lane.form-field", "Lane")
                .control("scene.form.lane")
                .description("Each lane keeps the colour it is known by.")
                .child(
                    SegmentedControl::new("scene.form.lane")
                        .label("Lane")
                        .segments([
                            Segment::new("read", "Read")
                                .tint(super::display::identity_tint(&theme, "agent.read")),
                            Segment::new("shell", "Shell")
                                .tint(super::display::identity_tint(&theme, "agent.shell")),
                            Segment::new("network", "Network")
                                .tint(super::display::identity_tint(&theme, "agent.network")),
                        ])
                        .selected("shell")
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
            FormField::new("scene.form.region.form-field", "All Agent models")
                .control("scene.form.region")
                .description("Choose a chat-capable model for direct runs.")
                .child(region),
        )
        .into_any_element()
}

/// The password in the canonical sign-in composition, kept across frames.
pub(super) struct SceneAuthSignIn {
    identity: Entity<TextInput>,
    password: Entity<PasswordInput>,
}

impl Global for SceneAuthSignIn {}

pub(super) fn ensure_auth_sign_in(window: &mut Window, cx: &mut App) {
    if cx.has_global::<SceneAuthSignIn>() {
        return;
    }
    // Nobody signs in with a password alone, and a password field reviewed
    // without the field above it is reviewed in a shape no product ships.
    let identity = cx.new(|cx| {
        TextInput::new("scene.auth.sign-in.identity", window, cx)
            .placeholder("you@example.com")
            .text("ada@origingame.dev")
    });
    let password = cx.new(|cx| {
        PasswordInput::new("scene.auth.sign-in.password", window, cx)
            .name("Password")
            .placeholder("Enter password")
            .required(true)
    });
    cx.set_global(SceneAuthSignIn { identity, password });
}

pub(super) fn auth_sign_in(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_auth_sign_in(window, cx);
    let identity = cx.global::<SceneAuthSignIn>().identity.clone();
    let password = cx.global::<SceneAuthSignIn>().password.clone();
    let theme = cx.theme().clone();
    let card_id = "scene.auth.sign-in.card";
    let heading = div()
        .column()
        .gap_token(&theme, Space::Xs)
        .child(
            crate::foundation::text(&theme, TypeScale::Title, "Sign in").semantic_in(
                cx,
                NodeSpec::new("scene.auth.sign-in.title", Role::Text)
                    .parent(card_id)
                    .text("Sign in"),
            ),
        )
        .child(
            crate::foundation::text(
                &theme,
                TypeScale::Body,
                "Continue to the workspace you were invited to.",
            )
            .text_color(theme.colors.text_muted)
            .semantic_in(
                cx,
                NodeSpec::new("scene.auth.sign-in.subtitle", Role::Text)
                    .parent(card_id)
                    .text("Continue to the workspace you were invited to."),
            ),
        );

    stack(&theme)
        .w(px(440.0))
        .child(
            Card::new().id(card_id).padded(true).child(
                div()
                    .column()
                    .gap_token(&theme, Space::Md)
                    .child(heading)
                    .child(
                        Callout::new("Credentials are verified by the caller.", Tone::Info)
                            .id("scene.auth.sign-in.boundary"),
                    )
                    .child(
                        FormField::new("scene.auth.sign-in.identity.field", "Email")
                            .control("scene.auth.sign-in.identity")
                            .required(true)
                            .child(identity),
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
                            Tone::Info,
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
                    MenuItem::command("publish.draft", "Save as draft")
                        .icon(Icon::Document)
                        .shortcut("cmd-s"),
                    MenuItem::command("publish.schedule", "Schedule…")
                        .icon(Icon::Calendar)
                        .shortcut("cmd-shift-s"),
                    MenuItem::command("publish.export", "Export without publishing")
                        .icon(Icon::ArchiveUp)
                        .shortcut("cmd-e"),
                    MenuItem::separator("publish.rule"),
                    MenuItem::command("publish.discard", "Discard this draft")
                        .icon(Icon::Trash)
                        .destructive(true),
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
                    // One row, one weight: a chipless button beside four
                    // chipped ones reads as the one that is unavailable.
                    IconButton::new("scene.actions.copy", Icon::Copy, "Copy run id")
                        .secondary()
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
                // A range picker built the way a host would build one: the
                // track is the group's, the answers that are not current stay
                // bare on it, and the one that is holds a chip in the colour
                // the library reserves for the current answer.
                ButtonGroup::new("scene.actions.range")
                    .children([
                        Button::new("scene.actions.range.day")
                            .label("Day")
                            .ghost()
                            .on_click(|_, _| {}),
                        Button::new("scene.actions.range.week")
                            .label("Week")
                            .variant(gpui_kit_theme::Variant::Light)
                            .selected(true)
                            .on_click(|_, _| {}),
                        Button::new("scene.actions.range.month")
                            .label("Month")
                            .ghost()
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
    asked: Entity<Pill>,
    told: Entity<Pill>,
}

/// A frame that changes shape around its text.
///
/// The area grows itself between `rows` and `max_rows`, which is all a field
/// standing in a column needs. This is the other case: a one-line pill that
/// becomes a panel once the message outgrows it. The decision belongs to the
/// frame, is taken before the area is laid out, and is about a width the area
/// is not currently in — so it is taken from [`Measured`] rather than from the
/// rows the area settled on.
pub(super) struct Pill {
    ident: Ident,
    area: Entity<TextArea>,
    /// How wide the text may be and still be a pill. Measured while it was
    /// one, because a panel is wider than the pill it replaced and cannot ask
    /// what would fit back there.
    room: Pixels,
    panel: bool,
    /// The pass the shape was last decided on. Changing shape changes the
    /// width, so the measurement that caused a change describes a frame that
    /// no longer exists.
    decided: u64,
}

impl Pill {
    fn new(
        ident: impl Into<Ident>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let ident = ident.into();
        let area = cx.new(|cx| {
            TextArea::new(ident.child("text"), window, cx)
                .text(text.to_string())
                .frame(Frame::Host)
                .enter(Enter::Submits)
                .autosize(1, 6)
        });
        Self {
            ident,
            area,
            room: px(0.0),
            panel: false,
            decided: 0,
        }
    }
}

impl Render for Pill {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        if let Some(measured) = self.area.read(cx).measured()
            && measured.pass > self.decided
        {
            if !self.panel {
                self.room = measured.wrapped;
            }
            let panel = measured.text > self.room;
            if panel != self.panel {
                self.panel = panel;
                self.decided = measured.pass;
            }
        }
        div()
            .id(self.ident.element_id())
            .w_full()
            .p(px(theme.space(Space::Xs)))
            .surface(&theme, Surface::Raised)
            .radius(
                &theme,
                match self.panel {
                    true => Radius::Card,
                    false => Radius::Pill,
                },
            )
            .child(self.area.clone())
    }
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
                    .autosize(3, 6)
                    .max_length(240)
            }),
            review: cx.new(|cx| {
                TextArea::new("scene.textarea.review", window, cx)
                    .placeholder("What changed, and why")
                    .autosize(3, 6)
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
                    .autosize(2, 8)
            }),
            asked: cx
                .new(|cx| Pill::new("scene.textarea.asked", "Rerun the failing test", window, cx)),
            told: cx.new(|cx| {
                Pill::new(
                    "scene.textarea.told",
                    "Rerun the failing test, and if it fails the same way again, \
                     bisect back to the commit that changed the fixture.",
                    window,
                    cx,
                )
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

    // A field with no label is a box. Each one is put where a product would
    // put it: under the words that say what it is for, and above the sentence
    // that says what is wrong with it.
    div()
        .flex()
        .flex_col()
        .gap(px(theme.space(Space::Md)))
        .p(px(theme.space(Space::Lg)))
        .w(px(360.0))
        .child(
            FormField::new("scene.input.token.field", "API token")
                .control("scene.input.token")
                .description("Kept on this machine, never published.")
                .child(token),
        )
        .child(
            FormField::new("scene.input.disabled.field", "Workspace id")
                .control("scene.input.disabled")
                .description("Set when the workspace was created.")
                .child(disabled),
        )
        .child(
            FormField::new("scene.input.invalid.field", "Email")
                .control("scene.input.invalid")
                .required(true)
                .error("This is not an address anyone can be reached at.")
                .child(invalid),
        )
        .child(
            FormField::new("scene.input.provider.field", "Provider")
                .control("scene.input.provider")
                .description("Where a run is sent.")
                .child(provider),
        )
        .into_any_element()
}

pub(super) fn textarea(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_inputs(window, cx);
    let inputs = cx.global::<SceneInputs>();
    let (notes, review, frozen, message, asked, told) = (
        inputs.notes.clone(),
        inputs.review.clone(),
        inputs.frozen.clone(),
        inputs.message.clone(),
        inputs.asked.clone(),
        inputs.told.clone(),
    );
    let theme = cx.theme().clone();

    div()
        .flex()
        .flex_col()
        .gap(px(theme.space(Space::Md)))
        .p(px(theme.space(Space::Lg)))
        .w(px(360.0))
        .child(
            FormField::new("scene.textarea.notes.field", "Release notes")
                .control("scene.textarea.notes")
                .description("Shown to everyone who opens this run.")
                .child(notes),
        )
        .child(
            FormField::new("scene.textarea.review.field", "Review")
                .control("scene.textarea.review")
                .child(review),
        )
        .child(
            FormField::new("scene.textarea.frozen.field", "Policy")
                .control("scene.textarea.frozen")
                .child(frozen),
        )
        // The other enter policy, for text that is a message rather than a
        // value. Nothing about it looks different, which is the point: what
        // changes is which key is the common act.
        .child(message)
        .child(caption(
            &theme,
            "a frame that measures its text rather than its rows: the same pill \
             holds one line, and becomes a panel for a message that outgrew it",
        ))
        .child(asked)
        .child(told)
        .into_any_element()
}

const EDITOR_SOURCE: &str = r#"use gpui::App;

pub fn summarize(values: &[u32]) -> Option<u32> {
    let total = values.iter().copied().sum();
    let message = "language policy stays with the caller";
    (total > 0).then_some(total)
}

const LONG_SOURCE_LINE: &str = "one source row stays whole without wrapping";
"#;

pub(super) struct SceneEditor {
    editor: Entity<Editor>,
}

impl Global for SceneEditor {}

pub(super) fn ensure_editor(window: &mut Window, cx: &mut App) {
    if cx.has_global::<SceneEditor>() {
        return;
    }
    let theme = cx.theme().clone();
    let span = |needle: &str, color| {
        let start = EDITOR_SOURCE
            .find(needle)
            .expect("scene source contains span");
        EditorHighlight::new(
            start..start + needle.len(),
            gpui::HighlightStyle {
                color: Some(color),
                ..Default::default()
            },
        )
    };
    let highlights = EditorHighlights::new(
        0,
        [
            span("use", theme.colors.syntax.get(SyntaxColor::Keyword)),
            span("pub fn", theme.colors.syntax.get(SyntaxColor::Keyword)),
            span("let total", theme.colors.syntax.get(SyntaxColor::Keyword)),
            span(
                "\"language policy stays with the caller\"",
                theme.colors.syntax.get(SyntaxColor::StringLiteral),
            ),
            span("0", theme.colors.syntax.get(SyntaxColor::Number)),
            span("const", theme.colors.syntax.get(SyntaxColor::Keyword)),
        ],
    );
    let editor = cx.new(|cx| {
        Editor::new(
            "scene.editor",
            "Rust source editor",
            EDITOR_SOURCE,
            window,
            cx,
        )
        .rows(12)
        .highlights(highlights)
        .indent_with(|request| {
            let caret = request.selection.end;
            match request.direction {
                EditorIndentDirection::Indent => Some(
                    EditorIndentation::new(caret..caret, "    ").selection(caret + 4..caret + 4),
                ),
                EditorIndentDirection::Outdent => None,
            }
        })
    });
    let area = editor.read(cx).text_area().clone();
    let caret = EDITOR_SOURCE
        .find("let message")
        .expect("scene source contains the focused line");
    area.update(cx, |area, cx| area.set_selected_range(caret..caret, cx));
    window.focus(&area.read(cx).focus_handle(cx), cx);
    cx.set_global(SceneEditor { editor });
}

pub(super) fn editor(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_editor(window, cx);
    let editor = cx.global::<SceneEditor>().editor.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(760.0))
        .child(caption(
            &theme,
            "one text/IME/history geometry; caller-owned revision highlights and indentation",
        ))
        .child(editor)
        .into_any_element()
}

pub(super) struct SceneMentionInput {
    input: Entity<MentionInput>,
}

impl Global for SceneMentionInput {}

pub(super) fn ensure_mention_input(window: &mut Window, cx: &mut App) {
    if cx.has_global::<SceneMentionInput>() {
        return;
    }
    let editor = cx.new(|cx| {
        TextArea::new("scene.mention.editor", window, cx)
            .text("Please ask @ad")
            .placeholder("Message the team")
            .enter(Enter::Submits)
            .rows(2)
    });
    let input = cx.new(|cx| {
        MentionInput::new("scene.mention", editor.clone(), cx).candidates([
            MentionCandidate::new("ada", "Ada Lovelace")
                .description("Compiler group")
                .replacement("@Ada"),
            MentionCandidate::new("adam", "Adam Stokes")
                .description("Release engineering")
                .replacement("@Adam"),
            MentionCandidate::new("admin", "Workspace administrators")
                .description("Group mention")
                .replacement("@admins")
                .unavailable("Group mentions are disabled here"),
        ])
    });
    window.focus(&editor.read(cx).focus_handle(cx), cx);
    cx.set_global(SceneMentionInput { input });
}

pub(super) fn mention_input(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_mention_input(window, cx);
    let input = cx.global::<SceneMentionInput>().input.clone();
    let theme = cx.theme().clone();
    div()
        .column()
        .gap_token(&theme, Space::Sm)
        .p_token(&theme, Space::Lg)
        .w(px(420.0))
        .child(caption(
            &theme,
            "the editor owns text; the anchored menu owns only @query completion",
        ))
        .child(input)
        .into_any_element()
}

pub(super) struct SceneRichTextEditor {
    editor: Entity<RichTextEditor>,
}

impl Global for SceneRichTextEditor {}

pub(super) fn ensure_rich_text_editor(window: &mut Window, cx: &mut App) {
    if cx.has_global::<SceneRichTextEditor>() {
        return;
    }
    let title_text = "A structured editing surface";
    let body_text = "Inline code stays quiet, links remain caller-owned, and diagnostics keep their actual severity.";
    let code_start = body_text
        .find("Inline code")
        .expect("fixture phrase exists");
    let code_end = code_start + "Inline code".len();
    let link_start = body_text.find("links").expect("fixture phrase exists");
    let link_end = link_start + "links".len();
    let document = RichTextDocument::new([
        RichTextBlock::new("rich-title", title_text).with_style(
            0..title_text.len(),
            RichTextInlineStyle::default().with_format(RichTextFormat::Bold, true),
        ),
        RichTextBlock::new("rich-body", body_text)
            .with_style(
                code_start..code_end,
                RichTextInlineStyle::default().with_format(RichTextFormat::Code, true),
            )
            .with_style(
                link_start..link_end,
                RichTextInlineStyle::default()
                    .with_link(Some("https://example.invalid/policy".into())),
            ),
        RichTextBlock::new("rich-list-one", "Host owns persistence and collaboration.")
            .with_paragraph(
                RichTextParagraphStyle::default()
                    .with_list(Some(RichTextListItem::new(RichTextListKind::Unordered))),
            ),
        RichTextBlock::new(
            "rich-list-two",
            "Kit owns selection, IME, layout, and formatting.",
        )
        .with_paragraph(
            RichTextParagraphStyle::default()
                .with_list(Some(RichTextListItem::new(RichTextListKind::Unordered))),
        ),
        RichTextBlock::new("rich-centered", "Alignment shares caret geometry.").with_paragraph(
            RichTextParagraphStyle::default().with_alignment(RichTextAlignment::Center),
        ),
    ])
    .expect("fixture document is valid");
    let session = cx.new(|_| RichTextEditSession::new(document));
    let next_id = Rc::new(std::cell::Cell::new(0_u64));
    let editor = cx.new(|cx| {
        let next_id = Rc::clone(&next_id);
        RichTextEditor::new(
            "scene.rich-text-editor",
            session,
            move || {
                let value = next_id.get().wrapping_add(1);
                next_id.set(value);
                RichTextBlockId::new(format!("scene-rich-{value}"))
            },
            window,
            cx,
        )
        .name("Structured document")
        .rows(8)
        .max_rows(8)
        .diagnostics([RichTextDiagnostic::new(
            RichTextRange {
                start: RichTextPosition::new("rich-body", link_start),
                end: RichTextPosition::new("rich-body", link_end),
            },
            RichTextDiagnosticSeverity::Warning,
        )])
    });
    cx.set_global(SceneRichTextEditor { editor });
}

pub(super) fn rich_text_editor(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_rich_text_editor(window, cx);
    let editor = cx.global::<SceneRichTextEditor>().editor.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(720.0))
        .child(caption(
            &theme,
            "one caller-owned document; styled blocks, lists, diagnostics, IME, and semantics share one projection",
        ))
        .child(editor)
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
                            .hint("PDF, PNG, or plain text")
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
    let general = || {
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
                    .search_terms(["engine", "local executor"])
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
            )
    };
    let sync = || {
        SettingsSection::new("scene.settings.sync", "Synchronisation")
            .description("What travels between machines")
            .dimmed_by("This workspace is local, so nothing synchronises.")
            .row(
                SettingsRow::new("scene.settings.sync.settings", "Sync settings")
                    .description("Keyboard, theme, and editor preferences")
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
                    .description("Runs and transcripts from the last 30 days")
                    .value("Off")
                    .control(
                        Switch::new("scene.settings.sync.history.switch")
                            .named("Sync history")
                            .on(false)
                            .on_change(|_, _, _| {}),
                    ),
            )
    };

    stack(&theme)
        .w_full()
        .child(
            row(&theme)
                .w_full()
                .items_start()
                // The filtered list opens with its own count, so the
                // unfiltered one is given a line saying what it is. Without
                // it the two pages start at different heights and nothing in
                // the picture says why one column sits lower than the other.
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .column()
                        .gap_token(&theme, Space::Md)
                        .child(caption(&theme, "no query, so every section"))
                        .child(SettingsList::new("scene.settings.all").section(general())),
                )
                .child(
                    div().flex_1().min_w_0().child(
                        SettingsList::new("scene.settings.filtered")
                            .query("sync")
                            .section(general())
                            .section(sync()),
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
                .recent([theme.colors.accent, theme.colors.success])
                .on_change(|_, _, _| {}),
        )
        .child(caption(&theme, "a swatch reports the colour it was given"))
        .child(
            row(&theme)
                .items_start()
                .child(
                    div()
                        .column()
                        .items_center()
                        .gap_token(&theme, Space::Xs)
                        .child(ColorSwatch::new(
                            "scene.color.swatch.accent",
                            theme.colors.accent,
                        ))
                        .child(caption(&theme, "Default")),
                )
                .child(
                    div()
                        .column()
                        .items_center()
                        .gap_token(&theme, Space::Xs)
                        .child(
                            ColorSwatch::new("scene.color.swatch.selected", current)
                                .selected(true)
                                .on_click(|_, _, _| {}),
                        )
                        .child(caption(&theme, "Selected")),
                )
                .child(
                    div()
                        .column()
                        .items_center()
                        .gap_token(&theme, Space::Xs)
                        .child(
                            ColorSwatch::new("scene.color.swatch.disabled", theme.colors.danger)
                                .disabled(true),
                        )
                        .child(caption(&theme, "Disabled")),
                ),
        )
        .into_any_element()
}
