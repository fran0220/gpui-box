//! Sections that disclose their body when the caller says they are open.
//!
//! Which sections are open is caller-owned. The accordion reports the section
//! that was activated and the state it should take next; it shows exactly the
//! set the caller passed, so a host that refuses to open a section leaves it
//! closed.

use std::f32::consts::FRAC_PI_2;
use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Transformation, Window, div, prelude::FluentBuilder, px,
    radians,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{
    ActiveTheme, ControlSize, Elevation, Radius, Space, Surface, TextTone, TypeScale,
};

use crate::display::icon::flips;
use crate::foundation::direction::{ActiveDirection, DirectionalExt};
use crate::foundation::{
    FocusRing, Ident, Pressable, Sizable, StyledExt, rule, text as foundation_text,
};
use crate::layout::measure;
use crate::motion;

type ToggleHandler = Rc<dyn Fn(SharedString, bool, &mut Window, &mut App)>;

/// One disclosure section: a header the typist can operate and a body that
/// exists only while the section is open.
pub struct AccordionSection {
    id: SharedString,
    title: SharedString,
    description: Option<SharedString>,
    disabled: bool,
    body: Option<AnyElement>,
}

impl std::fmt::Debug for AccordionSection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AccordionSection")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("disabled", &self.disabled)
            .field("has_body", &self.body.is_some())
            .finish()
    }
}

impl AccordionSection {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            disabled: false,
            body: None,
        }
    }

    /// Secondary text in the header, readable while the section is closed.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn body(mut self, body: impl IntoElement) -> Self {
        self.body = Some(body.into_any_element());
        self
    }
}

/// A stack of disclosure sections.
#[derive(IntoElement)]
pub struct Accordion {
    ident: Ident,
    sections: Vec<AccordionSection>,
    expanded: Vec<SharedString>,
    exclusive: bool,
    size: ControlSize,
    on_toggle: Option<ToggleHandler>,
}

impl std::fmt::Debug for Accordion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Accordion")
            .field("ident", &self.ident)
            .field("sections", &self.sections.len())
            .field("expanded", &self.expanded)
            .field("exclusive", &self.exclusive)
            .field("has_handler", &self.on_toggle.is_some())
            .finish()
    }
}

impl Accordion {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            sections: Vec::new(),
            expanded: Vec::new(),
            exclusive: false,
            size: ControlSize::Md,
            on_toggle: None,
        }
    }

    pub fn section(mut self, section: AccordionSection) -> Self {
        self.sections.push(section);
        self
    }

    pub fn sections(mut self, sections: impl IntoIterator<Item = AccordionSection>) -> Self {
        self.sections.extend(sections);
        self
    }

    pub fn expanded(mut self, ids: impl IntoIterator<Item = SharedString>) -> Self {
        self.expanded = ids.into_iter().collect();
        self
    }

    pub fn expanded_ids<S: AsRef<str>>(mut self, ids: &[S]) -> Self {
        self.expanded = ids
            .iter()
            .map(|id| SharedString::from(id.as_ref().to_string()))
            .collect();
        self
    }

    /// Whether opening a section also reports a close for every other open
    /// section.
    ///
    /// Exclusivity changes only what is reported, never what is shown: the
    /// accordion always renders the set the caller passed to
    /// [`Accordion::expanded`], so a host that applies only part of the report
    /// gets exactly what it applied.
    pub fn exclusive(mut self, exclusive: bool) -> Self {
        self.exclusive = exclusive;
        self
    }

    pub fn on_toggle(
        mut self,
        handler: impl Fn(SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }
}

impl Sizable for Accordion {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Accordion {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let expanded_ids = self.expanded.clone();
        let direction = cx.layout_direction();

        let mut stack = div()
            .column()
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Panel, Elevation::Raised)
            .overflow_hidden();

        for (position, section) in self.sections.into_iter().enumerate() {
            let open = expanded_ids.contains(&section.id);
            let actionable = !section.disabled && self.on_toggle.is_some();
            let ident = self.ident.child(section.id.as_ref());
            let color = if section.disabled {
                theme.colors.text_faint
            } else {
                theme.colors.text
            };

            let mut header = div()
                .id(ident.element_id())
                .row_reading(direction)
                .w_full()
                .gap(px(theme.space(Space::Sm)))
                .px(px(metrics.padding_x))
                .py(px(theme.space(Space::Sm)))
                .child(
                    icon(Icon::AltArrowRight)
                        .size(px(metrics.icon_size))
                        .text_color(theme.colors.text_muted)
                        .when(open, |glyph| {
                            glyph.with_transformation(Transformation::rotate(radians(FRAC_PI_2)))
                        })
                        // Open, it points down, and down is down either way.
                        .when(!open && flips(Icon::AltArrowRight, direction), |glyph| {
                            glyph.with_transformation(Transformation::scale(gpui::size(-1.0, 1.0)))
                        }),
                )
                .child(
                    div()
                        .column()
                        .flex_1()
                        .gap(px(theme.space(Space::Xs)))
                        .child(
                            foundation_text(&theme, TypeScale::Label, section.title.clone())
                                .when(open, |title| {
                                    title.font_weight(gpui::FontWeight(
                                        theme.typography.strong.weight,
                                    ))
                                })
                                .text_size(px(metrics.font_size))
                                .text_start(direction)
                                .text_color(color),
                        )
                        .children(section.description.clone().map(|description| {
                            foundation_text(&theme, TypeScale::Caption, description)
                                .text_start(direction)
                                .text_tone(&theme, TextTone::Muted)
                        })),
                )
                .when(section.disabled, |element| {
                    element.opacity(theme.opacity.disabled)
                })
                .when(actionable, |element| {
                    element
                        .cursor_pointer()
                        .tab_index(0)
                        .pressable(cx)
                        .hover(|style| style.bg(theme.colors.hover))
                        .focus_ring(&theme)
                });

            if let (true, Some(handler)) = (actionable, self.on_toggle.clone()) {
                let reports = reports(&expanded_ids, &section.id, open, self.exclusive);
                let key_reports = reports.clone();
                let click = Rc::clone(&handler);
                header = header
                    .on_click(move |_, window, cx| {
                        for (id, next) in &reports {
                            click(id.clone(), *next, window, cx);
                        }
                    })
                    .on_key_down(move |event, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            for (id, next) in &key_reports {
                                handler(id.clone(), *next, window, cx);
                            }
                            cx.stop_propagation();
                        }
                    });
            }

            let header = header.semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Button)
                    .parent(self.ident.semantic_id())
                    .expanded(open)
                    .disabled(section.disabled)
                    .text(section.title.clone()),
            );

            let body_id = ident.child("body").semantic_id();
            let disclosed = motion::tracked(
                &body_id,
                f32::from(u8::from(open)),
                motion::resize(&theme),
                window,
                cx,
            );
            // A section that is not disclosing at all drops its body entirely
            // rather than hiding it, so nothing invisible stays addressable.
            // While a section is still collapsing its body is still on screen,
            // and something on screen is something a typist can point at, so
            // it stays addressable exactly as long as it stays visible.
            let body = (disclosed > 0.0).then_some(section.body).flatten();
            let measured = measure::cell(&body_id, window, cx);
            let height = px(f32::from(measured.get().size.height) * disclosed);

            // A stack of sections with no lines in it is one slab with several
            // paragraphs on it. The rule is what says where one section ends,
            // and it goes between them rather than around the stack, which
            // already has a card's edge.
            if position > 0 {
                stack = stack.child(rule(&theme));
            }
            stack = stack.child(div().column().child(header).children(body.map(|body| {
                let content = div()
                    // A disclosed body is a different plane from the header
                    // that opened it, so an open section reads as opened
                    // rather than as two paragraphs of one header.
                    .surface(&theme, Surface::Sunken)
                    // Indented to the title rather than to the
                    // chevron, so the body reads as belonging to the
                    // section it hangs under.
                    .ps(
                        direction,
                        px(metrics.padding_x + metrics.icon_size + theme.space(Space::Sm)),
                    )
                    .pe(direction, px(metrics.padding_x))
                    .py(px(theme.space(Space::Sm)))
                    // The header already turns with the reading order, and a
                    // body that does not turn with it reads as belonging to
                    // the section on the other side. The row has to turn as
                    // well as the text: a shrink-to-fit child is placed by the
                    // flex axis, which `text_align` does not reach.
                    .row_reading(direction)
                    .text_start(direction)
                    .child(body);
                let record = {
                    let measured = Rc::clone(&measured);
                    move |bounds: Vec<gpui::Bounds<gpui::Pixels>>,
                          window: &mut Window,
                          _: &mut App| {
                        if let Some(first) = bounds.first() {
                            measure::record(&measured, *first, window);
                        }
                    }
                };

                // A settled section is laid out exactly as it was
                // before there was any motion here: the body sits in
                // the flow and its own height is the section's height.
                // Only a section in flight uses the driven height, and
                // it takes the body out of the flow to get one, so the
                // measurement stays the body's natural height instead
                // of chasing the frame being animated around it.
                if disclosed >= 1.0 {
                    div().w_full().on_children_prepainted(record).child(content)
                } else {
                    div()
                        .relative()
                        .w_full()
                        .h(height)
                        .overflow_hidden()
                        .on_children_prepainted(record)
                        .child(content.absolute().top_0().left_0().right_0())
                }
            })));
        }

        stack.semantic_in(cx, NodeSpec::new(self.ident.semantic_id(), Role::Group))
    }
}

/// What activating `id` reports.
///
/// Under exclusivity, opening a section also reports a close for every other
/// open one, which is a report about intent and not a change to what is drawn.
fn reports(
    expanded: &[SharedString],
    id: &SharedString,
    open: bool,
    exclusive: bool,
) -> Vec<(SharedString, bool)> {
    let mut reports = vec![(id.clone(), !open)];
    if exclusive && !open {
        reports.extend(
            expanded
                .iter()
                .filter(|other| *other != id)
                .map(|other| (other.clone(), false)),
        );
    }
    reports
}
