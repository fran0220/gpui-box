//! What a cell's colour means, once a theme is in the room.
//!
//! Ported from `crabtalk/bezel` (MIT); see `PROVENANCE.md`. The sixteen named
//! slots are no longer a table here — they are `color.terminal.ansi` in the
//! token files, because a repeated semantic colour belongs to the token
//! authority and a per-theme palette compiled into a component is exactly the
//! second source of truth that authority exists to prevent. What survives as
//! code is the arithmetic above index 15, which is arithmetic in the protocol
//! rather than a decision anyone gets to make.

use gpui::{Hsla, Rgba};
use gpui_kit_theme::{Appearance, Theme};

use super::emulator::CellColor;

/// The component levels of the xterm colour cube.
const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

/// Resolve an indexed colour to components, for an appearance.
///
/// Three ranges, and they are not the same kind of thing:
///
/// - **0-15** are *names*. A program asking for "red" is asking the terminal
///   what red is, which is why these come from the theme's tokens and change
///   with it.
/// - **16-231** is the 6x6x6 cube, where index 196 means `#ff0000` by
///   arithmetic. Remapping it would be inventing a colour the caller chose
///   against, so both appearances leave it alone.
/// - **232-255** is the grey ramp, and programs reach for it to say *dimmer*
///   rather than to name a grey. Its dark-to-light direction only reads as
///   dimmer on a dark background, so the light appearance mirrors it: 232 is
///   the faintest and 255 the strongest either way. Without the mirror the
///   ramp's bright end — where most hint text lands — is the end that vanishes
///   on white.
fn indexed_rgb(appearance: Appearance, index: u8) -> Option<(u8, u8, u8)> {
    match index {
        // Named, and therefore the theme's to answer.
        0..=15 => None,
        16..=231 => {
            let offset = index as usize - 16;
            Some((
                CUBE_LEVELS[offset / 36],
                CUBE_LEVELS[(offset / 6) % 6],
                CUBE_LEVELS[offset % 6],
            ))
        }
        232..=255 => {
            let step = index - 232;
            let step = match appearance {
                Appearance::Dark => step,
                Appearance::Light => 23 - step,
            };
            let level = 8 + 10 * step;
            Some((level, level, level))
        }
    }
}

pub(crate) fn rgb8(r: u8, g: u8, b: u8) -> Hsla {
    Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
    .into()
}

/// The colour to paint a cell in.
pub(crate) fn resolve(color: CellColor, theme: &Theme) -> Hsla {
    match color {
        CellColor::Foreground => theme.colors.text,
        CellColor::Background => theme.colors.terminal_background,
        CellColor::Indexed(index) => match indexed_rgb(theme.appearance, index) {
            Some((r, g, b)) => rgb8(r, g, b),
            None => theme.colors.terminal_ansi[index as usize],
        },
        CellColor::Rgb(r, g, b) => rgb8(r, g, b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_named_slots_are_the_themes_to_answer() {
        for index in 0..16u8 {
            assert_eq!(indexed_rgb(Appearance::Dark, index), None);
        }
        let dark = Theme::studio_dark();
        let light = Theme::studio_light();
        for index in 0..16u8 {
            assert_eq!(
                resolve(CellColor::Indexed(index), &dark),
                dark.colors.terminal_ansi[index as usize]
            );
            assert_ne!(
                resolve(CellColor::Indexed(index), &light),
                resolve(CellColor::Indexed(index), &dark),
                "slot {index} is tuned for its background, not shared"
            );
        }
    }

    #[test]
    fn the_cube_is_arithmetic_and_the_same_in_both_appearances() {
        assert_eq!(indexed_rgb(Appearance::Dark, 16), Some((0, 0, 0)));
        assert_eq!(indexed_rgb(Appearance::Dark, 231), Some((255, 255, 255)));
        assert_eq!(indexed_rgb(Appearance::Dark, 196), Some((255, 0, 0)));
        assert_eq!(indexed_rgb(Appearance::Dark, 46), Some((0, 255, 0)));
        assert_eq!(indexed_rgb(Appearance::Dark, 21), Some((0, 0, 255)));
        for index in 16..=231u8 {
            assert_eq!(
                indexed_rgb(Appearance::Dark, index),
                indexed_rgb(Appearance::Light, index),
                "the caller picked this colour by number"
            );
        }
    }

    #[test]
    fn the_grey_ramp_mirrors_so_dim_stays_dim() {
        assert_eq!(indexed_rgb(Appearance::Dark, 232), Some((8, 8, 8)));
        assert_eq!(indexed_rgb(Appearance::Dark, 255), Some((238, 238, 238)));
        assert_eq!(indexed_rgb(Appearance::Light, 232), Some((238, 238, 238)));
        assert_eq!(indexed_rgb(Appearance::Light, 255), Some((8, 8, 8)));

        // The property, stated once: the faint end of the ramp is the end
        // closest to the background it sits on, in both appearances.
        let dark = Theme::studio_dark();
        let light = Theme::studio_light();
        let distance = |theme: &Theme, index: u8| {
            (resolve(CellColor::Indexed(index), theme).l - theme.colors.terminal_background.l).abs()
        };
        assert!(distance(&dark, 232) < distance(&dark, 255));
        assert!(distance(&light, 232) < distance(&light, 255));
    }

    #[test]
    fn the_defaults_follow_the_theme_and_a_direct_color_does_not() {
        let theme = Theme::studio_dark();
        assert_eq!(resolve(CellColor::Foreground, &theme), theme.colors.text);
        assert_eq!(
            resolve(CellColor::Background, &theme),
            theme.colors.terminal_background
        );
        assert_eq!(
            resolve(CellColor::Rgb(255, 0, 0), &theme),
            rgb8(255, 0, 0),
            "a program that named a colour gets that colour"
        );
    }
}
