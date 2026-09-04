//! What a host gets when it says nothing about words, and what it gets when
//! it says something.
//!
//! Every assertion here reads the semantic tree, never the source, so what is
//! proved is that a component asked the catalogue for a string rather than
//! that a particular English word appears somewhere in this repository.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, Entity, IntoElement, SharedString, TestAppContext};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

/// The accessible name a node publishes, which is what a reader is read.
fn shown(harness: &mut Harness, id: &str) -> String {
    harness
        .node(id)
        .unwrap_or_else(|| panic!("semantic node `{id}` is missing"))
        .text
        .unwrap_or_else(|| panic!("`{id}` publishes no text"))
        .to_string()
}

fn pagination(cx: &mut TestAppContext) -> Harness {
    Harness::new(cx, gpui_kit::install, |_, _| {
        Pagination::new("results.pages")
            .page(2)
            .total(PageTotal::Known(9))
            .on_select(|_, _, _| {})
            .into_any_element()
    })
}

#[gpui::test]
fn a_host_that_says_nothing_gets_english(cx: &mut TestAppContext) {
    let mut harness = pagination(cx);
    assert_eq!(shown(&mut harness, "results.pages.next"), "Next page");
    assert_eq!(shown(&mut harness, "results.pages.status"), "Page 2 of 9");
}

#[gpui::test]
fn a_host_override_reaches_the_screen(cx: &mut TestAppContext) {
    let mut harness = pagination(cx);
    harness.update(|_, cx| {
        set_strings(
            [
                (StringKey::PaginationNext, "Seite vor".into()),
                (StringKey::PaginationPageOfTotal, "Seite {0} von {1}".into()),
            ],
            cx,
        );
    });

    assert_eq!(shown(&mut harness, "results.pages.next"), "Seite vor");
    // The values still land where the sentence puts them, which is the point
    // of numbering the placeholders rather than substituting in order.
    assert_eq!(shown(&mut harness, "results.pages.status"), "Seite 2 von 9");
}

#[gpui::test]
fn a_translation_may_reorder_the_values_it_is_given(cx: &mut TestAppContext) {
    let mut harness = pagination(cx);
    harness.update(|_, cx| {
        set_strings(
            [(StringKey::PaginationPageOfTotal, "{1} pages; on {0}".into())],
            cx,
        );
    });
    assert_eq!(shown(&mut harness, "results.pages.status"), "9 pages; on 2");
}

#[derive(Debug)]
struct MarkedNumbers;

impl NumberAdapter for MarkedNumbers {
    fn count(&self, value: usize) -> SharedString {
        format!("№{value}").into()
    }

    fn plural(&self, value: usize) -> Plural {
        match value {
            1 => Plural::One,
            3 => Plural::Few,
            _ => Plural::Other,
        }
    }

    fn count_of_total(&self, done: usize, total: usize) -> SharedString {
        format!("№{done} / №{total}").into()
    }

    fn percent(&self, value: f32) -> SharedString {
        format!("{} pct", value * 100.0).into()
    }

    fn decimal(&self, value: f64, precision: usize) -> SharedString {
        format!("{value:.*}", precision).into()
    }

    fn at_least(&self, value: usize) -> SharedString {
        format!("≥№{value}").into()
    }

    fn dimensions(&self, width: usize, height: usize) -> SharedString {
        format!("№{height} ↔ №{width}").into()
    }

    fn ordinal(&self, value: u64) -> SharedString {
        format!("№{value}º").into()
    }

    fn signed_count(&self, value: isize) -> SharedString {
        format!("Δ№{value}").into()
    }
}

#[gpui::test]
fn a_component_reads_numbers_from_the_installed_adapter(cx: &mut TestAppContext) {
    let mut harness = pagination(cx);
    harness.update(|_, cx| set_numbers(MarkedNumbers, cx));

    assert_eq!(shown(&mut harness, "results.pages.status"), "Page №2 of №9");
}

#[gpui::test]
fn plural_categories_the_component_does_not_name_reach_its_phrase(cx: &mut TestAppContext) {
    let slot: Rc<RefCell<Option<Entity<SearchField>>>> = Rc::new(RefCell::new(None));
    let build_slot = Rc::clone(&slot);
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    let mut field = SearchField::new("find", window, cx);
                    field.set_count(
                        HitCount::Known {
                            total: 3,
                            current: None,
                        },
                        cx,
                    );
                    field
                })
            })
            .clone()
            .into_any_element()
    });
    harness.update(|_, cx| {
        set_numbers(MarkedNumbers, cx);
        set_plural_strings(
            [(StringKey::SearchHitMany, Plural::Few, "{0} wyniki".into())],
            cx,
        );
    });

    assert_eq!(shown(&mut harness, "find.count"), "№3 wyniki");
}

#[test]
fn an_adapter_owns_each_reusable_number_shape() {
    let numbers = MarkedNumbers;
    assert_eq!(numbers.at_least(12).as_ref(), "≥№12");
    assert_eq!(numbers.dimensions(1920, 1080).as_ref(), "№1080 ↔ №1920");
    assert_eq!(numbers.ordinal(12).as_ref(), "№12º");
    assert_eq!(numbers.signed_count(-12).as_ref(), "Δ№-12");
}

#[gpui::test]
fn a_partial_override_leaves_the_rest_in_english(cx: &mut TestAppContext) {
    let mut harness = pagination(cx);
    harness.update(|_, cx| {
        set_strings([(StringKey::PaginationNext, "Seite vor".into())], cx);
    });

    assert_eq!(shown(&mut harness, "results.pages.next"), "Seite vor");
    // Nothing blanks: an entry nobody replaced still answers.
    assert_eq!(
        shown(&mut harness, "results.pages.previous"),
        "Previous page"
    );
    assert_eq!(shown(&mut harness, "results.pages.first"), "First page");
}

#[gpui::test]
fn clearing_an_override_puts_the_english_back(cx: &mut TestAppContext) {
    let mut harness = pagination(cx);
    harness.update(|_, cx| {
        set_strings([(StringKey::PaginationNext, "Seite vor".into())], cx);
    });
    harness.update(|_, cx| reset_strings(cx));
    assert_eq!(shown(&mut harness, "results.pages.next"), "Next page");
}

#[gpui::test]
fn a_word_with_no_business_identity_still_comes_from_the_catalogue(cx: &mut TestAppContext) {
    // The overflow group is named by the library, not by the caller: nobody
    // passed it a label, so it is exactly the case a literal would hide in.
    let mut harness = Harness::new(cx, gpui_kit::install, |window, cx| {
        let menu = cx.new(|cx| Menu::new("editor.overflow", window, cx).trigger("More"));
        Toolbar::new("editor.toolbar")
            .group(
                "actions",
                [
                    ToolbarItem::new(
                        "editor.undo",
                        "Undo",
                        Button::new("editor.undo").label("Undo").on_click(|_, _| {}),
                    ),
                    ToolbarItem::new(
                        "editor.redo",
                        "Redo",
                        Button::new("editor.redo").label("Redo").on_click(|_, _| {}),
                    ),
                ],
            )
            .overflow_after(1)
            .overflow_menu(menu)
            .into_any_element()
    });

    assert_eq!(
        shown(&mut harness, "editor.toolbar.overflow"),
        "More actions"
    );
    harness.update(|_, cx| {
        set_strings([(StringKey::MoreActions, "Weitere Aktionen".into())], cx);
    });
    assert_eq!(
        shown(&mut harness, "editor.toolbar.overflow"),
        "Weitere Aktionen"
    );
}

#[gpui::test]
fn a_word_a_host_owns_outranks_the_catalogue(cx: &mut TestAppContext) {
    // A caller that names a control itself keeps that name: the catalogue is
    // a default, not an override of the host's own data.
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        SplitPane::new("workspace.split")
            .handle_label("Resize the editor")
            .into_any_element()
    });
    harness.update(|_, cx| {
        set_strings(
            [(StringKey::SplitResizeHandle, "Should not appear".into())],
            cx,
        );
    });
    assert_eq!(
        shown(&mut harness, "workspace.split.divider"),
        "Resize the editor"
    );
}

#[gpui::test]
fn a_component_reads_the_catalogue_even_with_no_global_installed(cx: &mut TestAppContext) {
    // A host that installs the theme but not the catalogue still gets words,
    // because a library that renders blank labels for want of a global would
    // be worse than one with English compiled in.
    let mut harness = Harness::new(
        cx,
        |cx| {
            gpui_kit::assets::register_fonts(cx);
            gpui_kit::theme::Theme::install(cx);
            gpui_kit::semantics::install(cx);
        },
        |_, _| {
            Pagination::new("bare.pages")
                .page(1)
                .total(PageTotal::Known(3))
                .on_select(|_, _, _| {})
                .into_any_element()
        },
    );
    assert_eq!(shown(&mut harness, "bare.pages.next"), "Next page");
}
