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
use gpui_kit_theme::{ActiveTheme, Radius, Space, TypeScale};

use crate::foundation::{Ident, StyledExt};

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

impl RenderOnce for Accordion {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(gpui_kit_theme::ControlSize::Md);
        let last = self.sections.len().saturating_sub(1);
        let expanded_ids = self.expanded.clone();

        let mut stack = div()
            .column()
            .radius(&theme, Radius::Card)
            .hairline(&theme)
            .overflow_hidden();

        for (index, section) in self.sections.into_iter().enumerate() {
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
                .row()
                .w_full()
                .gap(px(theme.space(Space::Sm)))
                .px(px(metrics.padding_x))
                .py(px(theme.space(Space::Sm)))
                .text_size(px(metrics.font_size))
                .text_color(color)
                .child(
                    icon(Icon::AltArrowRight)
                        .size(px(metrics.icon_size))
                        .text_color(theme.colors.text_muted)
                        .when(open, |glyph| {
                            glyph.with_transformation(Transformation::rotate(radians(FRAC_PI_2)))
                        }),
                )
                .child(
                    div()
                        .column()
                        .flex_1()
                        .gap(px(2.0))
                        .child(section.title.clone())
                        .children(section.description.clone().map(|description| {
                            div()
                                .type_scale(&theme, TypeScale::Caption)
                                .text_color(theme.colors.text_muted)
                                .child(description)
                        })),
                )
                .when(section.disabled, |element| {
                    element.opacity(theme.opacity.disabled)
                })
                .when(actionable, |element| {
                    element
                        .cursor_pointer()
                        .tab_index(0)
                        .hover(|style| style.bg(theme.colors.hover))
                        .focus(|style| style.shadow(theme.selected_ring()))
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

            // A closed section drops its body entirely rather than hiding it,
            // so nothing invisible stays addressable.
            let body = open.then_some(section.body).flatten();

            stack = stack.child(
                div()
                    .column()
                    .when(index != last, |element| {
                        element
                            .border_b(px(theme.borders.hairline))
                            .border_color(theme.colors.hairline)
                    })
                    .child(header)
                    .children(body.map(|body| {
                        div()
                            // Indented to the title rather than to the
                            // chevron, so the body reads as belonging to the
                            // section it hangs under.
                            .pl(px(metrics.padding_x
                                + metrics.icon_size
                                + theme.space(Space::Sm)))
                            .pr(px(metrics.padding_x))
                            .pb(px(theme.space(Space::Sm)))
                            .text_size(px(metrics.font_size))
                            .text_color(theme.colors.text_muted)
                            .child(body)
                    })),
            );
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
