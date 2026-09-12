use crate::{App, Bounds, Context, Entity, InputHandler, Pixels, UTF16Selection, Window};
use std::ops::Range;

/// Implement this trait to allow views to handle textual input when implementing an editor, field, etc.
///
/// Once your view implements this trait, you can use it to construct an [`ElementInputHandler<V>`].
/// This input handler can then be assigned during paint by calling [`Window::handle_input`].
///
/// See [`InputHandler`] for details on how to implement each method.
pub trait EntityInputHandler: 'static + Sized {
    /// See [`InputHandler::native_selection`].
    fn native_selection(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<crate::NativeTextSelection> {
        None
    }

    /// See [`InputHandler::set_native_selection`].
    fn set_native_selection(
        &mut self,
        _selection: crate::NativeTextSelection,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> bool {
        false
    }

    /// See [`InputHandler::native_position_in_direction`].
    fn native_position_in_direction(
        &mut self,
        _position: crate::NativeTextPosition,
        _direction: crate::TextNavigationDirection,
        _offset: usize,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<crate::NativeTextPosition> {
        None
    }

    /// See [`InputHandler::native_position_bounds`].
    fn native_position_bounds(
        &mut self,
        _position: crate::NativeTextPosition,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        None
    }

    /// See [`InputHandler::native_position_for_point`].
    fn native_position_for_point(
        &mut self,
        _point: crate::Point<Pixels>,
        _within_range: Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<crate::NativeTextPosition> {
        None
    }

    /// See [`InputHandler::farthest_native_position`].
    fn farthest_native_position(
        &mut self,
        _range: Range<usize>,
        _direction: crate::TextNavigationDirection,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<crate::NativeTextPosition> {
        None
    }

    /// See [`InputHandler::selection_rects_for_range`].
    fn selection_rects_for_range(
        &mut self,
        _range: Range<usize>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Vec<crate::TextSelectionRect> {
        Vec::new()
    }

    /// See [`InputHandler::text_position_in_direction`].
    fn text_position_in_direction(
        &mut self,
        _position: usize,
        _direction: crate::TextNavigationDirection,
        _offset: usize,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }

    /// See [`InputHandler::farthest_text_position`].
    fn farthest_text_position(
        &mut self,
        _range: Range<usize>,
        _direction: crate::TextNavigationDirection,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }

    /// See [`InputHandler::base_writing_direction`].
    fn base_writing_direction(
        &mut self,
        _position: usize,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<crate::TextWritingDirection> {
        None
    }

    /// See [`InputHandler::set_base_writing_direction`].
    fn set_base_writing_direction(
        &mut self,
        _direction: crate::TextWritingDirection,
        _range: Range<usize>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> bool {
        false
    }

    /// See [`InputHandler::grapheme_range_at`].
    fn grapheme_range_at(
        &mut self,
        _position: usize,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        None
    }

    /// See [`InputHandler::native_caret_bounds`].
    fn native_caret_bounds(
        &mut self,
        _position: usize,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        None
    }

    /// See [`InputHandler::text_input_options`].
    fn text_input_options(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> crate::TextInputOptions {
        crate::TextInputOptions::default()
    }

    /// See [`InputHandler::perform_text_input_action`].
    fn perform_text_input_action(
        &mut self,
        _action: crate::TextInputAction,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> bool {
        false
    }

    /// See [`InputHandler::text_for_range`] for details
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<String>;

    /// See [`InputHandler::selected_text_range`] for details
    fn selected_text_range(
        &mut self,
        ignore_disabled_input: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<UTF16Selection>;

    /// See [`InputHandler::marked_text_range`] for details
    fn marked_text_range(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Range<usize>>;

    /// See [`InputHandler::unmark_text`] for details
    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>);

    /// See [`InputHandler::replace_text_in_range`] for details
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    );

    /// See [`InputHandler::replace_and_mark_text_in_range`] for details
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    );

    /// See [`InputHandler::bounds_for_range`] for details
    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>>;

    /// See [`InputHandler::character_index_for_point`] for details
    fn character_index_for_point(
        &mut self,
        point: crate::Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize>;

    /// See [`InputHandler::set_selected_text_range`] for details
    fn set_selected_text_range(
        &mut self,
        _range_utf16: Range<usize>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    /// See [`InputHandler::text_length_utf16`] for details
    fn text_length_utf16(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }

    /// See [`InputHandler::accepts_text_input`] for details
    fn accepts_text_input(&self, _window: &mut Window, _cx: &mut Context<Self>) -> bool {
        true
    }
}

/// The canonical implementation of [`crate::PlatformInputHandler`]. Call [`Window::handle_input`]
/// with an instance during your element's paint.
pub struct ElementInputHandler<V> {
    view: Entity<V>,
    element_bounds: Bounds<Pixels>,
    visual_transform: crate::VisualTransform,
}

impl<V: 'static> ElementInputHandler<V> {
    /// Used in [`Element::paint`][element_paint] with the element's bounds, a `Window`, and a `App` context.
    ///
    /// [element_paint]: crate::Element::paint
    pub fn new(element_bounds: Bounds<Pixels>, view: Entity<V>) -> Self {
        ElementInputHandler {
            view,
            element_bounds,
            visual_transform: crate::VisualTransform::default(),
        }
    }

    /// Captures the mapping used by this frame's logical `element_bounds`.
    /// Only the handler's element-bounds metadata is mapped here. Entity callback
    /// points remain window-global, returned caret/range rectangles must already
    /// be displayed window coordinates, and `bounds_for_range` still receives
    /// logical element bounds. The entity must store the same prepaint mapping
    /// with its layout snapshot and inverse-map incoming points exactly once.
    pub fn with_visual_transform(mut self, transform: crate::VisualTransform) -> Self {
        self.visual_transform = transform;
        self
    }
}

impl<V: EntityInputHandler> InputHandler for ElementInputHandler<V> {
    fn native_selection(
        &mut self,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<crate::NativeTextSelection> {
        self.view
            .update(cx, |view, cx| view.native_selection(window, cx))
    }
    fn set_native_selection(
        &mut self,
        selection: crate::NativeTextSelection,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        self.view.update(cx, |view, cx| {
            view.set_native_selection(selection, window, cx)
        })
    }
    fn native_position_in_direction(
        &mut self,
        position: crate::NativeTextPosition,
        direction: crate::TextNavigationDirection,
        offset: usize,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<crate::NativeTextPosition> {
        self.view.update(cx, |view, cx| {
            view.native_position_in_direction(position, direction, offset, window, cx)
        })
    }
    fn native_position_bounds(
        &mut self,
        position: crate::NativeTextPosition,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Bounds<Pixels>> {
        self.view.update(cx, |view, cx| {
            view.native_position_bounds(position, window, cx)
        })
    }
    fn native_position_for_point(
        &mut self,
        point: crate::Point<Pixels>,
        within_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<crate::NativeTextPosition> {
        self.view.update(cx, |view, cx| {
            view.native_position_for_point(point, within_range, window, cx)
        })
    }
    fn farthest_native_position(
        &mut self,
        range: Range<usize>,
        direction: crate::TextNavigationDirection,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<crate::NativeTextPosition> {
        self.view.update(cx, |view, cx| {
            view.farthest_native_position(range, direction, window, cx)
        })
    }

    fn selection_rects_for_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<crate::TextSelectionRect> {
        self.view.update(cx, |view, cx| {
            view.selection_rects_for_range(range, window, cx)
        })
    }

    fn text_position_in_direction(
        &mut self,
        position: usize,
        direction: crate::TextNavigationDirection,
        offset: usize,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<usize> {
        self.view.update(cx, |view, cx| {
            view.text_position_in_direction(position, direction, offset, window, cx)
        })
    }

    fn farthest_text_position(
        &mut self,
        range: Range<usize>,
        direction: crate::TextNavigationDirection,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<usize> {
        self.view.update(cx, |view, cx| {
            view.farthest_text_position(range, direction, window, cx)
        })
    }

    fn base_writing_direction(
        &mut self,
        position: usize,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<crate::TextWritingDirection> {
        self.view.update(cx, |view, cx| {
            view.base_writing_direction(position, window, cx)
        })
    }

    fn set_base_writing_direction(
        &mut self,
        direction: crate::TextWritingDirection,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        self.view.update(cx, |view, cx| {
            view.set_base_writing_direction(direction, range, window, cx)
        })
    }

    fn grapheme_range_at(
        &mut self,
        position: usize,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Range<usize>> {
        self.view
            .update(cx, |view, cx| view.grapheme_range_at(position, window, cx))
    }

    fn native_caret_bounds(
        &mut self,
        position: usize,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Bounds<Pixels>> {
        self.view.update(cx, |view, cx| {
            view.native_caret_bounds(position, window, cx)
        })
    }

    fn text_input_options(&mut self, window: &mut Window, cx: &mut App) -> crate::TextInputOptions {
        self.view
            .update(cx, |view, cx| view.text_input_options(window, cx))
    }

    fn perform_text_input_action(
        &mut self,
        action: crate::TextInputAction,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        self.view.update(cx, |view, cx| {
            view.perform_text_input_action(action, window, cx)
        })
    }

    fn selected_text_range(
        &mut self,
        ignore_disabled_input: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<UTF16Selection> {
        self.view.update(cx, |view, cx| {
            view.selected_text_range(ignore_disabled_input, window, cx)
        })
    }

    fn marked_text_range(&mut self, window: &mut Window, cx: &mut App) -> Option<Range<usize>> {
        self.view
            .update(cx, |view, cx| view.marked_text_range(window, cx))
    }

    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<String> {
        self.view.update(cx, |view, cx| {
            view.text_for_range(range_utf16, adjusted_range, window, cx)
        })
    }

    fn replace_text_in_range(
        &mut self,
        replacement_range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.view.update(cx, |view, cx| {
            view.replace_text_in_range(replacement_range, text, window, cx)
        });
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.view.update(cx, |view, cx| {
            view.replace_and_mark_text_in_range(
                range_utf16,
                new_text,
                new_selected_range,
                window,
                cx,
            )
        });
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut App) {
        self.view
            .update(cx, |view, cx| view.unmark_text(window, cx));
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Bounds<Pixels>> {
        self.view.update(cx, |view, cx| {
            view.bounds_for_range(range_utf16, self.element_bounds, window, cx)
        })
    }

    fn character_index_for_point(
        &mut self,
        point: crate::Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<usize> {
        self.view.update(cx, |view, cx| {
            view.character_index_for_point(point, window, cx)
        })
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.view.update(cx, |view, cx| {
            view.set_selected_text_range(range_utf16, window, cx)
        })
    }

    fn element_bounds(&mut self, _window: &mut Window, _cx: &mut App) -> Option<Bounds<Pixels>> {
        Some(self.visual_transform.map_bounds(self.element_bounds))
    }

    fn text_length_utf16(&mut self, window: &mut Window, cx: &mut App) -> Option<usize> {
        self.view
            .update(cx, |view, cx| view.text_length_utf16(window, cx))
    }

    fn accepts_text_input(&mut self, window: &mut Window, cx: &mut App) -> bool {
        self.view
            .update(cx, |view, cx| view.accepts_text_input(window, cx))
    }

    fn prefers_ime_for_printable_keys(&mut self, window: &mut Window, cx: &mut App) -> bool {
        self.view
            .update(cx, |view, cx| view.accepts_text_input(window, cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AppContext, FocusHandle, IntoElement, PlatformWindow, Render, TestAppContext,
        TextInputAction, TextInputOptions, TextInputStateChange, TextNavigationDirection,
        TextSelectionRect, TextWritingDirection, canvas, point, px, size,
    };

    struct NativeInputProbe {
        focus: FocusHandle,
        accepts: bool,
        options: TextInputOptions,
        selection: crate::NativeTextSelection,
    }

    impl Render for NativeInputProbe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let focus = self.focus.clone();
            let view = cx.entity();
            canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| {
                    window.handle_input(&focus, ElementInputHandler::new(bounds, view), cx);
                },
            )
        }
    }

    impl EntityInputHandler for NativeInputProbe {
        fn native_selection(
            &mut self,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<crate::NativeTextSelection> {
            Some(self.selection)
        }
        fn set_native_selection(
            &mut self,
            selection: crate::NativeTextSelection,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> bool {
            if selection.head.utf16_offset > 20 {
                return false;
            }
            self.selection = selection;
            true
        }
        fn native_position_in_direction(
            &mut self,
            position: crate::NativeTextPosition,
            direction: TextNavigationDirection,
            offset: usize,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<crate::NativeTextPosition> {
            assert_eq!(
                position,
                crate::NativeTextPosition {
                    utf16_offset: 7,
                    affinity: crate::TextAffinity::Upstream
                }
            );
            assert_eq!((direction, offset), (TextNavigationDirection::Left, 3));
            Some(crate::NativeTextPosition {
                utf16_offset: 7,
                affinity: crate::TextAffinity::Downstream,
            })
        }
        fn native_position_bounds(
            &mut self,
            position: crate::NativeTextPosition,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<Bounds<Pixels>> {
            assert_eq!(
                position,
                crate::NativeTextPosition {
                    utf16_offset: 7,
                    affinity: crate::TextAffinity::Upstream
                }
            );
            Some(Bounds::new(point(px(23.), px(41.)), size(px(1.), px(17.))))
        }
        fn native_position_for_point(
            &mut self,
            position: crate::Point<Pixels>,
            within_range: Option<Range<usize>>,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<crate::NativeTextPosition> {
            assert_eq!(position, point(px(-23.), px(71.)));
            if let Some(range) = within_range {
                assert_eq!(range, 2..9);
                Some(crate::NativeTextPosition {
                    utf16_offset: 7,
                    affinity: crate::TextAffinity::Upstream,
                })
            } else {
                None
            }
        }
        fn farthest_native_position(
            &mut self,
            range: Range<usize>,
            direction: TextNavigationDirection,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<crate::NativeTextPosition> {
            assert_eq!((range, direction), (2..9, TextNavigationDirection::Right));
            Some(crate::NativeTextPosition {
                utf16_offset: 4,
                affinity: crate::TextAffinity::Upstream,
            })
        }
        fn text_input_options(
            &mut self,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> TextInputOptions {
            self.options
        }
        fn accepts_text_input(&self, _: &mut Window, _: &mut Context<Self>) -> bool {
            self.accepts
        }
        fn perform_text_input_action(
            &mut self,
            action: TextInputAction,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> bool {
            action == TextInputAction::Search
        }
        fn selection_rects_for_range(
            &mut self,
            range: Range<usize>,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Vec<TextSelectionRect> {
            assert_eq!(range, 2..9);
            vec![TextSelectionRect {
                bounds: Bounds::new(point(px(17.), px(29.)), size(px(41.), px(13.))),
                writing_direction: TextWritingDirection::RightToLeft,
                contains_start: true,
                contains_end: false,
                is_vertical: false,
            }]
        }
        fn text_position_in_direction(
            &mut self,
            position: usize,
            direction: TextNavigationDirection,
            offset: usize,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<usize> {
            assert_eq!(
                (position, direction, offset),
                (7, TextNavigationDirection::Left, 3)
            );
            Some(12)
        }
        fn farthest_text_position(
            &mut self,
            range: Range<usize>,
            direction: TextNavigationDirection,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<usize> {
            assert_eq!((range, direction), (2..9, TextNavigationDirection::Right));
            Some(4)
        }
        fn base_writing_direction(
            &mut self,
            position: usize,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<TextWritingDirection> {
            assert_eq!(position, 7);
            Some(TextWritingDirection::RightToLeft)
        }
        fn grapheme_range_at(
            &mut self,
            position: usize,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<Range<usize>> {
            assert_eq!(position, 3);
            Some(2..7)
        }
        fn native_caret_bounds(
            &mut self,
            position: usize,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<Bounds<Pixels>> {
            assert_eq!(position, 9);
            Some(Bounds::new(point(px(71.), px(13.)), size(px(1.), px(19.))))
        }
        fn text_for_range(
            &mut self,
            _: Range<usize>,
            _: &mut Option<Range<usize>>,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<String> {
            None
        }
        fn selected_text_range(
            &mut self,
            _: bool,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<UTF16Selection> {
            None
        }
        fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
            None
        }
        fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {}
        fn replace_text_in_range(
            &mut self,
            _: Option<Range<usize>>,
            _: &str,
            _: &mut Window,
            _: &mut Context<Self>,
        ) {
        }
        fn replace_and_mark_text_in_range(
            &mut self,
            _: Option<Range<usize>>,
            _: &str,
            _: Option<Range<usize>>,
            _: &mut Window,
            _: &mut Context<Self>,
        ) {
        }
        fn bounds_for_range(
            &mut self,
            _: Range<usize>,
            _: Bounds<Pixels>,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<Bounds<Pixels>> {
            None
        }
        fn character_index_for_point(
            &mut self,
            _: crate::Point<Pixels>,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<usize> {
            None
        }
    }

    #[gpui::test]
    fn visual_transform_maps_input_metadata_but_not_entity_callback_coordinates(
        cx: &mut TestAppContext,
    ) {
        let handle = cx.add_window(|_, cx| NativeInputProbe {
            focus: cx.focus_handle(),
            accepts: true,
            options: TextInputOptions::default(),
            selection: crate::NativeTextSelection::default(),
        });
        handle
            .update(cx, |_, window, cx| {
                let view = cx.new(|cx| NativeInputProbe {
                    focus: cx.focus_handle(),
                    accepts: true,
                    options: TextInputOptions::default(),
                    selection: crate::NativeTextSelection::default(),
                });
                let mut input = ElementInputHandler::new(
                    Bounds::new(point(px(20.), px(30.)), size(px(40.), px(15.))),
                    view,
                )
                .with_visual_transform(crate::VisualTransform::scale_about(
                    1.5,
                    point(px(10.), px(4.)),
                ));
                assert_eq!(
                    input.element_bounds(window, cx),
                    Some(Bounds::new(
                        point(px(25.), px(43.)),
                        size(px(60.), px(22.5))
                    ))
                );
                let position = crate::NativeTextPosition {
                    utf16_offset: 7,
                    affinity: crate::TextAffinity::Upstream,
                };
                assert_eq!(
                    input.native_position_for_point(
                        point(px(-23.), px(71.)),
                        Some(2..9),
                        window,
                        cx
                    ),
                    Some(position)
                );
                assert_eq!(
                    input.native_position_bounds(position, window, cx),
                    Some(Bounds::new(point(px(23.), px(41.)), size(px(1.), px(17.))))
                );
                assert_eq!(
                    input.selection_rects_for_range(2..9, window, cx)[0].bounds,
                    Bounds::new(point(px(17.), px(29.)), size(px(41.), px(13.)))
                );
            })
            .expect("input mapping");
    }

    #[gpui::test]
    fn native_input_forwarding_preserves_values_and_refusals(cx: &mut TestAppContext) {
        let options = TextInputOptions {
            purpose: crate::KeyboardPurpose::Email,
            action: TextInputAction::Search,
            autofill: Some(crate::AutofillPurpose::Username),
            multiline: false,
            secure: true,
        };
        let window = cx.add_window(|window, cx| {
            let focus = cx.focus_handle();
            focus.focus(window, cx);
            NativeInputProbe {
                focus,
                accepts: true,
                options,
                selection: crate::NativeTextSelection::default(),
            }
        });
        cx.update_window(window.into(), |_, window, cx| {
            window.draw(cx).clear(cx);
        })
        .expect("paint input");
        let mut platform = cx.test_window(window.into());
        let mut input = platform.take_input_handler().expect("installed input");
        assert_eq!(input.text_input_options(), Some(options));
        assert!(input.perform_text_input_action(TextInputAction::Search));
        assert!(!input.perform_text_input_action(TextInputAction::Next));
        let upstream = crate::NativeTextPosition {
            utf16_offset: 7,
            affinity: crate::TextAffinity::Upstream,
        };
        let downstream = crate::NativeTextPosition {
            utf16_offset: 7,
            affinity: crate::TextAffinity::Downstream,
        };
        let selection = crate::NativeTextSelection {
            anchor: downstream,
            head: upstream,
        };
        assert!(input.set_native_selection(selection));
        assert_eq!(input.native_selection(), Some(selection));
        assert!(!input.set_native_selection(crate::NativeTextSelection {
            head: crate::NativeTextPosition {
                utf16_offset: 99,
                ..upstream
            },
            ..selection
        }));
        assert_eq!(input.native_selection(), Some(selection));
        assert_eq!(
            input.native_position_in_direction(upstream, TextNavigationDirection::Left, 3),
            Some(downstream)
        );
        assert_eq!(
            input.native_position_bounds(upstream),
            Some(Bounds::new(point(px(23.), px(41.)), size(px(1.), px(17.))))
        );
        assert_eq!(
            input.farthest_native_position(2..9, TextNavigationDirection::Right),
            Some(crate::NativeTextPosition {
                utf16_offset: 4,
                affinity: crate::TextAffinity::Upstream
            })
        );
        let rects = input.selection_rects_for_range(2..9);
        assert_eq!(
            input.native_position_for_point(point(px(-23.), px(71.)), Some(2..9)),
            Some(upstream)
        );
        assert_eq!(
            input.native_position_for_point(point(px(-23.), px(71.)), None),
            None
        );
        assert_eq!(rects.len(), 1);
        assert_eq!(
            rects[0].bounds,
            Bounds::new(point(px(17.), px(29.)), size(px(41.), px(13.)))
        );
        assert_eq!(
            rects[0].writing_direction,
            TextWritingDirection::RightToLeft
        );
        assert!(rects[0].contains_start && !rects[0].contains_end && !rects[0].is_vertical);
        assert_eq!(
            input.text_position_in_direction(7, TextNavigationDirection::Left, 3),
            Some(12)
        );
        assert_eq!(
            input.farthest_text_position(2..9, TextNavigationDirection::Right),
            Some(4)
        );
        assert_eq!(
            input.base_writing_direction(7),
            Some(TextWritingDirection::RightToLeft)
        );
        assert!(!input.set_base_writing_direction(TextWritingDirection::LeftToRight, 2..9));
        assert_eq!(input.grapheme_range_at(3), Some(2..7));
        assert_eq!(
            input.native_caret_bounds(9),
            Some(Bounds::new(point(px(71.), px(13.)), size(px(1.), px(19.))))
        );
    }

    #[gpui::test]
    fn native_keyboard_focus_and_options_notifications_do_not_repeat_each_frame(
        cx: &mut TestAppContext,
    ) {
        let window = cx.add_window(|window, cx| {
            let focus = cx.focus_handle();
            focus.focus(window, cx);
            NativeInputProbe {
                focus,
                accepts: true,
                options: TextInputOptions::default(),
                selection: crate::NativeTextSelection::default(),
            }
        });
        let platform = cx.test_window(window.into());
        platform.simulate_active_status_change(true);
        for _ in 0..2 {
            cx.update_window(window.into(), |_, window, cx| {
                window.draw(cx).clear(cx);
            })
            .expect("paint input");
        }
        assert_eq!(platform.0.lock().keyboard_requests, [true]);
        assert_eq!(
            platform.0.lock().text_input_changes,
            [TextInputStateChange::FocusGained]
        );
        window
            .update(cx, |view, _, cx| {
                view.options.action = TextInputAction::Next;
                cx.notify();
            })
            .expect("change hints");
        cx.update_window(window.into(), |_, window, cx| {
            window.draw(cx).clear(cx);
        })
        .expect("paint changed input");
        assert_eq!(
            platform.0.lock().text_input_changes,
            [
                TextInputStateChange::FocusGained,
                TextInputStateChange::OptionsChanged
            ]
        );
        window
            .update(cx, |view, _, cx| {
                view.accepts = false;
                cx.notify();
            })
            .expect("disable editor");
        cx.update_window(window.into(), |_, window, cx| {
            window.draw(cx).clear(cx);
        })
        .expect("paint disabled input");
        assert_eq!(platform.0.lock().keyboard_requests, [true, false]);
        assert_eq!(
            platform.0.lock().text_input_changes,
            [
                TextInputStateChange::FocusGained,
                TextInputStateChange::OptionsChanged,
                TextInputStateChange::FocusLost
            ]
        );
    }
}
