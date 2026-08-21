//! SGR sequences as readable text plus the colours they named.
//!
//! This is not a terminal. It recognises the sixteen theme ANSI colours, bold,
//! and reset, and it drops every other sequence rather than guessing at it.
//! The host still owns the bytes; this only separates the characters a reader
//! can see from the codes that coloured them.

use std::ops::Range;

use gpui::{FontWeight, HighlightStyle, Hsla, SharedString};
use gpui_kit_theme::Theme;

/// One coloured run of the stripped text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnsiRun {
    pub range: Range<usize>,
    /// An index into [`Theme::terminal_ansi`](gpui_kit_theme::Colors::terminal_ansi), when the run named a colour.
    pub color: Option<u8>,
    pub bold: bool,
}

/// Strips SGR sequences and returns the readable text plus the runs they
/// coloured.
pub fn strip_ansi(raw: &str) -> (SharedString, Vec<AnsiRun>) {
    let bytes = raw.as_bytes();
    let mut plain = String::new();
    let mut runs = Vec::new();
    let mut color = None;
    let mut bold = false;
    let mut run_start = 0usize;
    let mut index = 0usize;

    let flush = |plain: &String,
                 runs: &mut Vec<AnsiRun>,
                 run_start: &mut usize,
                 color: Option<u8>,
                 bold: bool| {
        let end = plain.len();
        if end > *run_start && (color.is_some() || bold) {
            runs.push(AnsiRun {
                range: *run_start..end,
                color,
                bold,
            });
        }
        *run_start = end;
    };

    while index < bytes.len() {
        if bytes[index] == 0x1b && bytes.get(index + 1) == Some(&b'[') {
            let Some((next, codes)) = read_sgr(&raw[index..]) else {
                if let Some(ch) = raw[index..].chars().next() {
                    plain.push(ch);
                    index += ch.len_utf8();
                } else {
                    break;
                }
                continue;
            };
            flush(&plain, &mut runs, &mut run_start, color, bold);
            apply_sgr(codes, &mut color, &mut bold);
            index += next;
            continue;
        }
        let Some(ch) = raw[index..].chars().next() else {
            break;
        };
        plain.push(ch);
        index += ch.len_utf8();
    }
    flush(&plain, &mut runs, &mut run_start, color, bold);
    (SharedString::from(plain), runs)
}

/// Highlight styles for the runs [`strip_ansi`] produced, using the theme's
/// sixteen ANSI colours.
pub fn ansi_highlights(runs: &[AnsiRun], theme: &Theme) -> Vec<(Range<usize>, HighlightStyle)> {
    runs.iter()
        .filter_map(|run| {
            let color = run
                .color
                .and_then(|index| theme.colors.terminal_ansi.get(index as usize).copied());
            if color.is_none() && !run.bold {
                return None;
            }
            Some((
                run.range.clone(),
                HighlightStyle {
                    color,
                    font_weight: run.bold.then_some(FontWeight::BOLD),
                    ..Default::default()
                },
            ))
        })
        .collect()
}

#[allow(dead_code)]
pub fn ansi_color(index: u8, theme: &Theme) -> Option<Hsla> {
    theme.colors.terminal_ansi.get(index as usize).copied()
}

fn read_sgr(text: &str) -> Option<(usize, Vec<u8>)> {
    let rest = text.strip_prefix("\u{1b}[")?;
    let end = rest.find('m')?;
    let body = &rest[..end];
    if !body
        .bytes()
        .all(|byte| byte.is_ascii_digit() || byte == b';')
    {
        return None;
    }
    let codes = if body.is_empty() {
        vec![0]
    } else {
        body.split(';')
            .filter_map(|part| part.parse::<u8>().ok())
            .collect()
    };
    Some((2 + end + 1, codes))
}

fn apply_sgr(codes: Vec<u8>, color: &mut Option<u8>, bold: &mut bool) {
    if codes.is_empty() {
        *color = None;
        *bold = false;
        return;
    }
    for code in codes {
        match code {
            0 => {
                *color = None;
                *bold = false;
            }
            1 => *bold = true,
            22 => *bold = false,
            30..=37 => *color = Some(code - 30),
            90..=97 => *color = Some(code - 90 + 8),
            39 => *color = None,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::strip_ansi;

    #[test]
    fn sgr_colours_leave_the_readable_text() {
        let (text, runs) = strip_ansi("\u{1b}[31mfailed\u{1b}[0m ok");
        assert_eq!(text.as_ref(), "failed ok");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].range, 0..6);
        assert_eq!(runs[0].color, Some(1));
    }

    #[test]
    fn an_unknown_sequence_is_left_in_the_text() {
        let (text, runs) = strip_ansi("\u{1b}[2Jkeep");
        assert_eq!(text.as_ref(), "\u{1b}[2Jkeep");
        assert!(runs.is_empty());
    }
}
