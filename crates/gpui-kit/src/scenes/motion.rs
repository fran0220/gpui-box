//! Procedural micro-motion, reviewed as itself.

use super::support::*;

pub(super) fn micro(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        .child(caption(
            &theme,
            "named functions of time; reduced motion leaves the glyph still",
        ))
        .child(
            row(&theme)
                .child(MicroMark::new(
                    "scene.micro.heartbeat",
                    Micro::Heartbeat,
                    "Hb",
                ))
                .child(MicroMark::new("scene.micro.bounce", Micro::Bounce, "Bn"))
                .child(MicroMark::new("scene.micro.wobble", Micro::Wobble, "Wb"))
                .child(MicroMark::new("scene.micro.pop", Micro::Pop, "Pp"))
                .child(MicroMark::new("scene.micro.sparkle", Micro::Sparkle, "Sk")),
        )
        .into_any_element()
}
