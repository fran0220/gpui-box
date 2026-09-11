//! A bounded page consumes system obscuration once around its fixed chrome
//! and shrinking content slot. Use Responsive outside it to select compact
//! bottom navigation or a desktop sidebar from measured container width.
use crate::foundation::Ident;
use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, Styled, Window,
    WindowInsets, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};

/// A full-height page with fixed header/footer and a bounded body. Insets are
/// residual obscuration in this viewport's logical pixels, not physical screen
/// coordinates. A keyboard that already resized the viewport has zero residual
/// overlap; floating keyboards must not be approximated by edge padding.
#[derive(IntoElement)]
pub struct PageLayout {
    ident: Ident,
    insets: Option<WindowInsets>,
    header: Option<AnyElement>,
    footer: Option<AnyElement>,
    body: AnyElement,
    hide_footer: bool,
}
impl PageLayout {
    /// Defaults to the window's current system insets (zero on desktop).
    /// Nested pages should override with zero after an ancestor consumed them.
    pub fn new(ident: impl Into<Ident>, body: impl IntoElement) -> Self {
        Self {
            ident: ident.into(),
            insets: None,
            header: None,
            footer: None,
            body: body.into_any_element(),
            hide_footer: false,
        }
    }
    pub fn insets(mut self, insets: WindowInsets) -> Self {
        self.insets = Some(insets);
        self
    }
    pub fn header(mut self, content: impl IntoElement) -> Self {
        self.header = Some(content.into_any_element());
        self
    }
    pub fn footer(mut self, content: impl IntoElement) -> Self {
        self.footer = Some(content.into_any_element());
        self
    }
    /// Explicit host policy, including when a resized keyboard leaves no IME
    /// overlap. Hidden footer controls are unmounted, not merely transparent.
    pub fn hide_footer(mut self, hidden: bool) -> Self {
        self.hide_footer = hidden;
        self
    }
}
impl RenderOnce for PageLayout {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let edges = self.insets.unwrap_or_else(|| window.insets()).effective();
        let parent = self.ident.semantic_id();
        div()
            .id(self.ident.element_id())
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .pt(edges.top.max(px(0.0)))
            .pr(edges.right.max(px(0.0)))
            .pb(edges.bottom.max(px(0.0)))
            .pl(edges.left.max(px(0.0)))
            .children(self.header.map(|header| div().flex_none().child(header)))
            .child(
                div()
                    .id(self.ident.child("body").element_id())
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .child(self.body)
                    .semantic_in(
                        cx,
                        NodeSpec::new(self.ident.child("body").semantic_id(), Role::Region)
                            .parent(parent.clone()),
                    ),
            )
            .children(
                self.footer
                    .filter(|_| !self.hide_footer)
                    .map(|footer| div().flex_none().child(footer)),
            )
            .semantic_in(cx, NodeSpec::new(parent, Role::Group))
    }
}
