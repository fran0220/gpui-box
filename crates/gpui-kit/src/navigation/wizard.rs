//! A multi-step flow that reports where the typist asked to go.
//!
//! Which step is current, which are done, and which cannot be reached are all
//! caller-owned. The wizard reports a navigation intent and moves nothing
//! itself, so a host that refuses to advance keeps showing the step that still
//! holds.
//!
//! A step that is blocked or has failed carries the reason it was given and
//! shows it. There is no bare grey dot standing in for "you cannot go here":
//! a refusal nobody can read is a refusal nobody can act on.

use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, Theme, TypeScale};

use crate::controls::button::Button;
use crate::display::badge::Tone;
use crate::foundation::stepping::bounded_step;
use crate::foundation::{Disableable, FocusRing, Ident, Pressable, Sizable, StyledExt};
use crate::motion;

/// How wide the marker beside a step is.
const MARKER: f32 = 20.0;

type NavigateHandler = Rc<dyn Fn(&WizardIntent, &mut Window, &mut App)>;

/// Where a step stands, as the caller reports it.
///
/// The wizard never derives this from position: a step is only current, done,
/// or refused because the host says so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepStatus {
    Complete,
    Current,
    Upcoming,
    /// Something has to happen elsewhere first, and this is what.
    Blocked(SharedString),
    /// The step was attempted and did not succeed, in the host's own words.
    Failed(SharedString),
}

impl StepStatus {
    /// The name a semantic node publishes, so a test asserts the state a step
    /// reported rather than the glyph it drew.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Current => "current",
            Self::Upcoming => "upcoming",
            Self::Blocked(_) => "blocked",
            Self::Failed(_) => "failed",
        }
    }

    /// Why the step cannot be taken, when that is a thing the host said.
    pub fn reason(&self) -> Option<&SharedString> {
        match self {
            Self::Blocked(reason) | Self::Failed(reason) => Some(reason),
            _ => None,
        }
    }

    fn tone(&self) -> Tone {
        match self {
            Self::Complete => Tone::Success,
            Self::Current => Tone::Accent,
            Self::Upcoming => Tone::Neutral,
            Self::Blocked(_) => Tone::Warning,
            Self::Failed(_) => Tone::Danger,
        }
    }

    fn glyph(&self) -> Option<Icon> {
        match self {
            Self::Complete => Some(Icon::Check),
            Self::Blocked(_) => Some(Icon::Key),
            Self::Failed(_) => Some(Icon::Danger),
            _ => None,
        }
    }
}

/// One step of a flow, identified by what it is rather than where it sits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WizardStep {
    id: SharedString,
    title: SharedString,
    description: Option<SharedString>,
    status: StepStatus,
    /// Whether the caller says this step may be jumped to. Unset means a
    /// completed step may be revisited and nothing else may be entered early.
    reachable: Option<bool>,
}

impl WizardStep {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            status: StepStatus::Upcoming,
            reachable: None,
        }
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn status(mut self, status: StepStatus) -> Self {
        self.status = status;
        self
    }

    pub fn complete(self) -> Self {
        self.status(StepStatus::Complete)
    }

    pub fn current(self) -> Self {
        self.status(StepStatus::Current)
    }

    pub fn upcoming(self) -> Self {
        self.status(StepStatus::Upcoming)
    }

    pub fn blocked(self, reason: impl Into<SharedString>) -> Self {
        self.status(StepStatus::Blocked(reason.into()))
    }

    pub fn failed(self, reason: impl Into<SharedString>) -> Self {
        self.status(StepStatus::Failed(reason.into()))
    }

    /// Whether the typist may jump straight to this step.
    ///
    /// Left unsaid, a completed step may be revisited and every other step may
    /// not, because a step nobody has reached is not a place the flow can
    /// honestly offer.
    pub fn reachable(mut self, reachable: bool) -> Self {
        self.reachable = Some(reachable);
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    fn is_current(&self) -> bool {
        self.status == StepStatus::Current
    }

    fn is_reachable(&self) -> bool {
        self.reachable
            .unwrap_or(matches!(self.status, StepStatus::Complete))
    }
}

/// Which way the steps are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WizardLayout {
    #[default]
    Horizontal,
    Vertical,
}

/// What a gesture asked the flow to do. The wizard applies none of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WizardIntent {
    /// Go straight to a named step.
    Step(SharedString),
    /// Return to the step the caller named as revisitable.
    Back,
    /// Move on from the current step.
    Next,
    /// End the flow, which is a different thing from moving on.
    Finish,
}

impl WizardIntent {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Step(_) => "step",
            Self::Back => "back",
            Self::Next => "next",
            Self::Finish => "finish",
        }
    }
}

/// A multi-step flow: the steps, the current step's body, and the way on.
#[derive(IntoElement)]
pub struct Wizard {
    ident: Ident,
    steps: Vec<WizardStep>,
    layout: WizardLayout,
    body: Option<AnyElement>,
    back_to: Option<SharedString>,
    finish: bool,
    can_advance: bool,
    back_label: SharedString,
    next_label: SharedString,
    finish_label: SharedString,
    size: ControlSize,
    disabled: bool,
    on_navigate: Option<NavigateHandler>,
}

impl std::fmt::Debug for Wizard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Wizard")
            .field("ident", &self.ident)
            .field("steps", &self.steps.len())
            .field("layout", &self.layout)
            .field("back_to", &self.back_to)
            .field("finish", &self.finish)
            .field("disabled", &self.disabled)
            .field("has_handler", &self.on_navigate.is_some())
            .finish()
    }
}

impl Wizard {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            steps: Vec::new(),
            layout: WizardLayout::default(),
            body: None,
            back_to: None,
            finish: false,
            can_advance: true,
            back_label: SharedString::new_static("Back"),
            next_label: SharedString::new_static("Next"),
            finish_label: SharedString::new_static("Finish"),
            size: ControlSize::Md,
            disabled: false,
            on_navigate: None,
        }
    }

    pub fn step(mut self, step: WizardStep) -> Self {
        self.steps.push(step);
        self
    }

    pub fn steps(mut self, steps: impl IntoIterator<Item = WizardStep>) -> Self {
        self.steps.extend(steps);
        self
    }

    pub fn layout(mut self, layout: WizardLayout) -> Self {
        self.layout = layout;
        self
    }

    pub fn vertical(self) -> Self {
        self.layout(WizardLayout::Vertical)
    }

    /// The current step's content, which belongs entirely to the caller.
    pub fn body(mut self, body: impl IntoElement) -> Self {
        self.body = Some(body.into_any_element());
        self
    }

    /// The earlier step the caller says may be returned to. Without one there
    /// is no back control at all.
    pub fn back_to(mut self, step: impl Into<SharedString>) -> Self {
        self.back_to = Some(step.into());
        self
    }

    /// Whether moving on from here ends the flow. Finishing is a different
    /// report from advancing, so it is a different control.
    pub fn finish(mut self, finish: bool) -> Self {
        self.finish = finish;
        self
    }

    /// Whether the flow may move on from the current step at all.
    pub fn can_advance(mut self, can_advance: bool) -> Self {
        self.can_advance = can_advance;
        self
    }

    pub fn back_label(mut self, label: impl Into<SharedString>) -> Self {
        self.back_label = label.into();
        self
    }

    pub fn next_label(mut self, label: impl Into<SharedString>) -> Self {
        self.next_label = label.into();
        self
    }

    pub fn finish_label(mut self, label: impl Into<SharedString>) -> Self {
        self.finish_label = label.into();
        self
    }

    pub fn on_navigate(
        mut self,
        handler: impl Fn(&WizardIntent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_navigate = Some(Rc::new(handler));
        self
    }

    fn handler(&self) -> Option<NavigateHandler> {
        self.on_navigate.clone().filter(|_| !self.disabled)
    }

    #[allow(clippy::too_many_arguments)]
    fn step_element(
        &self,
        step: &WizardStep,
        theme: &Theme,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let ident = self.ident.child(step.id.as_ref());
        let current = step.is_current();
        // A step nobody may jump to gets no handler, whatever it looks like.
        let actionable = !current && step.is_reachable() && self.handler().is_some();
        let tone = step.status.tone();
        let color = tone.color(theme);
        let vertical = self.layout == WizardLayout::Vertical;

        let filled = motion::tracked(
            &ident.semantic_id(),
            f32::from(u8::from(current || step.status == StepStatus::Complete)),
            motion::state_change(theme),
            window,
            cx,
        );

        let marker = div()
            .size(px(MARKER))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .border(px(theme.borders.hairline))
            .border_color(color.opacity(0.2 + 0.8 * filled))
            .bg(color.opacity(0.12 + 0.16 * filled))
            .text_color(color)
            .children(
                step.status
                    .glyph()
                    .map(|glyph| icon(glyph).size(px(MARKER * 0.55)).text_color(color)),
            )
            .when(step.status.glyph().is_none(), |element| {
                element.child(
                    div()
                        .size(px(MARKER * 0.3 * filled.max(0.5)))
                        .rounded_full()
                        .bg(color.opacity(0.3 + 0.7 * filled)),
                )
            });

        let reason = step.status.reason().map(|reason| {
            let failed = matches!(step.status, StepStatus::Failed(_));
            div()
                .type_scale(theme, TypeScale::Caption)
                .text_color(color)
                .child(reason.clone())
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.child("reason").semantic_id(), Role::Status)
                        .parent(ident.semantic_id())
                        .invalid(failed)
                        .text(reason.clone()),
                )
        });

        let text = div()
            .column()
            .min_w_0()
            .gap(px(2.0))
            .child(
                div()
                    .type_scale(theme, TypeScale::Label)
                    .text_color(if current {
                        theme.colors.text
                    } else {
                        theme.colors.text_muted
                    })
                    .child(step.title.clone()),
            )
            .children(step.description.clone().map(|description| {
                div()
                    .type_scale(theme, TypeScale::Caption)
                    .text_color(theme.colors.text_faint)
                    .child(description)
            }))
            .children(reason);

        let mut element = div()
            .id(ident.element_id())
            .row()
            .items_start()
            .gap_token(theme, Space::Sm)
            .p_token(theme, Space::Xs)
            .radius(theme, Radius::Control)
            .when(!vertical, |element| element.flex_1().min_w_0())
            .when(self.disabled, |element| {
                element.opacity(theme.opacity.disabled)
            })
            .when(actionable, |element| {
                element
                    .cursor_pointer()
                    .tab_index(0)
                    .pressable(cx)
                    .hover(|style| style.bg(theme.colors.hover))
                    .focus_ring(theme)
            })
            .child(marker)
            .child(text);

        if let (true, Some(handler)) = (actionable, self.handler()) {
            let id = step.id.clone();
            let click = Rc::clone(&handler);
            let clicked = id.clone();
            element = element
                .on_click(move |_, window, cx| {
                    click(&WizardIntent::Step(clicked.clone()), window, cx)
                })
                .on_key_down(move |event, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        handler(&WizardIntent::Step(id.clone()), window, cx);
                        cx.stop_propagation();
                    }
                });
        }

        element
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Tab)
                    .parent(self.ident.semantic_id())
                    .text(step.title.clone())
                    .selected(current)
                    .disabled(self.disabled || !actionable)
                    .value(step.status.as_str()),
            )
            .into_any_element()
    }
}

impl Disableable for Wizard {
    /// Refuses the whole flow: no step, no back, and no way on installs a
    /// handler.
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for Wizard {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Wizard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let vertical = self.layout == WizardLayout::Vertical;
        let count = self.steps.len();

        let mut strip = div()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .when(vertical, |element| element.column())
            .when(!vertical, |element| element.row().items_start());

        if let Some(handler) = self.handler() {
            let steps = self.steps.clone();
            strip = strip.on_key_down(move |event, window, cx| {
                let keys: [&str; 4] = if vertical {
                    ["up", "down", "home", "end"]
                } else {
                    ["left", "right", "home", "end"]
                };
                let key = event.keystroke.key.as_str();
                let from = steps.iter().position(WizardStep::is_current);
                let next = if key == keys[0] {
                    step_toward(&steps, from, -1)
                } else if key == keys[1] {
                    step_toward(&steps, from, 1)
                } else if key == keys[2] {
                    step_toward(&steps, None, 1)
                } else if key == keys[3] {
                    step_toward(&steps, None, -1)
                } else {
                    return;
                };
                let Some(next) = next else {
                    return;
                };
                handler(&WizardIntent::Step(next), window, cx);
                cx.stop_propagation();
            });
        }

        for step in &self.steps {
            strip = strip.child(self.step_element(step, &theme, window, cx));
        }

        let ident = self.ident.clone();
        let handler = self.handler();
        let body = self.body.map(|body| {
            div().w_full().child(body).semantic_in(
                cx,
                NodeSpec::new(ident.child("body").semantic_id(), Role::Group)
                    .parent(ident.semantic_id()),
            )
        });

        let back = handler
            .as_ref()
            .zip(self.back_to.clone())
            .map(|(handler, target)| {
                let handler = Rc::clone(handler);
                div()
                    .child(
                        Button::new(ident.child("back"))
                            .label(self.back_label.clone())
                            .secondary()
                            .control_size(self.size)
                            .semantic_parent(ident.semantic_id())
                            .on_click(move |window, cx| handler(&WizardIntent::Back, window, cx)),
                    )
                    .semantic_in(
                        cx,
                        NodeSpec::new(ident.child("back-target").semantic_id(), Role::Status)
                            .parent(ident.semantic_id())
                            .text("Returns to")
                            .value(target),
                    )
            });

        let advance = handler.as_ref().map(|handler| {
            let handler = Rc::clone(handler);
            let finish = self.finish;
            let button = ident.child(if finish { "finish" } else { "next" });
            let label = if finish {
                self.finish_label.clone()
            } else {
                self.next_label.clone()
            };
            Button::new(button)
                .label(label)
                .primary()
                .control_size(self.size)
                .semantic_parent(ident.semantic_id())
                .disabled(!self.can_advance)
                .on_click(move |window, cx| {
                    handler(
                        if finish {
                            &WizardIntent::Finish
                        } else {
                            &WizardIntent::Next
                        },
                        window,
                        cx,
                    )
                })
        });

        div()
            .column()
            .w_full()
            .gap_token(&theme, Space::Md)
            .child(strip)
            .children(body)
            .child(
                div()
                    .row()
                    .w_full()
                    .gap_token(&theme, Space::Sm)
                    .children(back)
                    .child(div().flex_1())
                    .children(advance),
            )
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::List)
                    .disabled(self.disabled)
                    .value(count.to_string()),
            )
    }
}

/// The next step in `delta`'s direction that may actually be jumped to.
///
/// Movement stops at the ends rather than wrapping, because a flow has a
/// beginning and an end.
fn step_toward(steps: &[WizardStep], from: Option<usize>, delta: isize) -> Option<SharedString> {
    bounded_step(steps.len(), from, delta, |index| {
        !steps[index].is_reachable()
    })
    .map(|index| steps[index].id.clone())
}
