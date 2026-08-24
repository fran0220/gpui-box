//! Procedural micro-motion, reviewed as itself.

use super::support::*;

pub(super) fn micro(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    // Each motion gets a cell of its own. A still frame cannot show any of
    // them moving, so what the picture has to carry instead is that there are
    // five of them, that they are the same size, and which name belongs to
    // which mark.
    let cell = |mark: MicroMark| {
        div()
            .w(px(96.0))
            .flex_none()
            .column()
            .items_center()
            .justify_center()
            .gap_token(&theme, Space::Xs)
            .py_token(&theme, Space::Md)
            .radius(&theme, Radius::Card)
            .well(&theme)
            .child(mark)
    };

    stack(&theme)
        .w(px(560.0))
        .child(caption(
            &theme,
            "named functions of time; reduced motion leaves the glyph still",
        ))
        .child(
            row(&theme)
                .gap_token(&theme, Space::Sm)
                .child(cell(MicroMark::new(
                    "scene.micro.heartbeat",
                    Micro::Heartbeat,
                    "Hb",
                )))
                .child(cell(MicroMark::new(
                    "scene.micro.bounce",
                    Micro::Bounce,
                    "Bn",
                )))
                .child(cell(MicroMark::new(
                    "scene.micro.wobble",
                    Micro::Wobble,
                    "Wb",
                )))
                .child(cell(MicroMark::new("scene.micro.pop", Micro::Pop, "Pp")))
                .child(cell(MicroMark::new(
                    "scene.micro.sparkle",
                    Micro::Sparkle,
                    "Sk",
                ))),
        )
        .child(caption(
            &theme,
            "the same functions on things that are not glyphs",
        ))
        .child(
            row(&theme)
                .gap_token(&theme, Space::Sm)
                .child(div().child(Badge::new("12 unread").neutral()).micro(
                    "scene.micro.badge",
                    Micro::Pop,
                    cx,
                ))
                .child(div().child(Tag::new("scene.micro.tag", "fixture")).micro(
                    "scene.micro.tag.mark",
                    Micro::Wobble,
                    cx,
                ))
                .child(div().child(StatusDot::new(Tone::Success)).micro(
                    "scene.micro.dot",
                    Micro::Heartbeat,
                    cx,
                )),
        )
        .into_any_element()
}
