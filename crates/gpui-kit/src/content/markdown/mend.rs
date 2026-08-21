//! Holding half-written inline markers steady until they finish arriving.
//!
//! `**bold` is not bold. It is four literal characters, and it stays four
//! literal characters until the closing `**` lands, at which point the markers
//! vanish, the run restyles, and every wrap point after it shifts. A reader
//! watching a reply arrive sees that as the paragraph twitching — once per
//! emphasis, once per code span, once per link.
//!
//! Closing the markers speculatively, for display only, removes the twitch:
//! the text is styled from the moment there is content to style, and the real
//! closer changes nothing when it comes. What the reader loses is a moment of
//! literal truth about a marker that has not closed yet; what they gain is a
//! paragraph that stops moving. When a marker genuinely never closes, the
//! settled parse says so, and there is exactly one correction rather than a
//! flicker throughout.
//!
//! The scan is deliberately approximate. It prefers being stable and close to
//! the final parse over resolving CommonMark's delimiter rules exactly,
//! because any mid-stream misjudgement is repaired by the next few characters
//! or by the settle. Two known quirks it accepts: `2**3` briefly bolds the
//! `3`, and an opener followed only by whitespace stays literal until real
//! content arrives.

/// Where a link whose address is still arriving points.
///
/// The address is never shown and never followed. A partial URL rendered as
/// text would collapse the line when the rest of it lands, and a partial URL
/// treated as a destination would be a link to somewhere nobody named.
pub(crate) const PENDING_LINK: &str = "pending:link";

/// One opener that has not been closed.
struct Open {
    marker: char,
    len: usize,
    /// Just past the run, which orders nesting and marks where content would
    /// have to appear for the opener to be worth closing.
    at: usize,
}

/// Speculatively closes whatever hangs open at the end of `text`.
///
/// `None` means nothing hangs, which is the common answer and costs one pass.
pub(crate) fn close_hanging(text: &str) -> Option<String> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let count = chars.len();
    let at = |index: usize| chars.get(index).map(|&(_, char)| char);

    let mut open: Vec<Open> = Vec::new();
    let mut brackets: Vec<usize> = Vec::new();
    // An open code span: how many backticks opened it, and where its content
    // starts.
    let mut code: Option<(usize, usize)> = None;
    // The last character that would justify closing an opener: not
    // whitespace, not another marker.
    let mut content: Option<usize> = None;
    // Where a link's address ran off the end of what has arrived.
    let mut unfinished: Option<usize> = None;

    let mut index = 0;
    while index < count {
        let char = chars[index].1;
        if code.is_none() && char == '\\' {
            // Both characters are literal, and the escaped one is content.
            if index + 1 < count {
                content = Some(index + 1);
            }
            index += 2;
            continue;
        }
        if char == '`' {
            let run = run_of(&chars, index);
            match code {
                // A span closes only on a run the length of the one that
                // opened it.
                Some((opened, _)) if run == opened => code = None,
                Some(_) => content = Some(index + run - 1),
                None => code = Some((run, index + run)),
            }
            index += run;
            continue;
        }
        if code.is_some() {
            content = Some(index);
            index += 1;
            continue;
        }
        match char {
            '*' | '_' | '~' => {
                let run = run_of(&chars, index);
                delimiter(&mut open, &chars, char, run, index, &mut content);
                index += run;
            }
            '[' => {
                brackets.push(index);
                index += 1;
            }
            ']' => {
                if let Some(opened) = brackets.pop() {
                    // Emphasis that opened inside a finished `[…]` and never
                    // closed there stays literal, which is what the settled
                    // parse will decide too.
                    open.retain(|hanging| hanging.at < opened);
                    if at(index + 1) == Some('(') {
                        let mut scan = index + 2;
                        let mut depth = 0usize;
                        loop {
                            match at(scan) {
                                Some('(') => depth += 1,
                                Some(')') if depth == 0 => break,
                                Some(')') => depth -= 1,
                                Some(_) => {}
                                None => {
                                    unfinished = Some(index);
                                    break;
                                }
                            }
                            scan += 1;
                        }
                        if unfinished.is_some() {
                            break;
                        }
                        content = Some(scan);
                        index = scan + 1;
                        continue;
                    }
                }
                content = Some(index);
                index += 1;
            }
            char if char.is_whitespace() => index += 1,
            _ => {
                content = Some(index);
                index += 1;
            }
        }
    }

    // The text stopped inside an address. Keep the link's words, point them
    // nowhere until the address is whole.
    if let Some(close) = unfinished {
        let byte = chars[close].0;
        return Some(format!("{}]({PENDING_LINK})", &text[..byte]));
    }

    // Innermost first. An open `[` sorts the emphasis closers by itself:
    // markers opened inside the link text close before its `](…)`, ones opened
    // before it close after.
    let mut closers: Vec<(usize, String)> = Vec::new();
    if let Some((ticks, from)) = code
        && content.is_some_and(|content| content >= from)
    {
        closers.push((from, "`".repeat(ticks)));
    }
    for hanging in &open {
        if content.is_some_and(|content| content >= hanging.at) {
            closers.push((hanging.at, hanging.marker.to_string().repeat(hanging.len)));
        }
    }
    if let Some(&opened) = brackets.last()
        && content.is_some_and(|content| content > opened)
    {
        closers.push((opened, format!("]({PENDING_LINK})")));
    }
    closers.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
    let closers: String = closers.into_iter().map(|(_, text)| text).collect();

    // A last line of nothing but one or two dashes under a line of text is an
    // underline to the parser and a list item that has just begun to almost
    // everyone else. A zero-width space breaks the reading invisibly, until
    // the next characters settle which it was.
    let underline = looks_like_underline(text);

    if closers.is_empty() && !underline {
        return None;
    }
    if underline {
        // The closers belong to the text above the underline, not to it.
        let line = text.rfind('\n');
        return Some(match (line, closers.is_empty()) {
            (Some(line), false) => {
                format!("{}{closers}{}\u{200B}", &text[..line], &text[line..])
            }
            _ => format!("{text}\u{200B}"),
        });
    }
    // Before any trailing whitespace: a closer with a space in front of it is
    // not closing anything.
    let end = text.trim_end().len();
    Some(format!("{}{closers}{}", &text[..end], &text[end..]))
}

fn run_of(chars: &[(usize, char)], index: usize) -> usize {
    let char = chars[index].1;
    chars[index..]
        .iter()
        .take_while(|&&(_, next)| next == char)
        .count()
}

/// Opens or closes one run of emphasis markers.
///
/// Closing takes the innermost opener of the same character, partially when
/// the closer is itself half-arrived (`**a*`). Markers opened after an opener
/// that just closed were inside the span it closed, and stay literal — which
/// is how the settled parse reads them.
fn delimiter(
    open: &mut Vec<Open>,
    chars: &[(usize, char)],
    marker: char,
    run: usize,
    index: usize,
    content: &mut Option<usize>,
) {
    let end = index + run;
    // Strikethrough is two tildes exactly; longer runs are literal. One tilde
    // may still be the first half of an arriving `~~`, so it reaches the
    // matcher, but it never opens.
    if marker == '~' && run > 2 {
        *content = Some(end - 1);
        return;
    }
    let before = index.checked_sub(1).map(|prior| chars[prior].1);
    let after = chars.get(end).map(|&(_, char)| char);
    let word = |char: Option<char>| char.is_some_and(char::is_alphanumeric);
    // An underscore inside a word never delimits, by the specification. A
    // single asterisk inside a word is treated the same way here, so that
    // `2*3` does not flash italic on its way to being arithmetic.
    if word(before) && word(after) && (marker == '_' || (marker == '*' && run == 1)) {
        *content = Some(end - 1);
        return;
    }
    let closes = before.is_some_and(|char| !char.is_whitespace());
    let opens = after.is_some_and(|char| !char.is_whitespace());
    let mut left = run;
    if closes && let Some(found) = open.iter().rposition(|hanging| hanging.marker == marker) {
        let taken = left.min(open[found].len);
        open[found].len -= taken;
        left -= taken;
        let keep = if open[found].len == 0 {
            found
        } else {
            found + 1
        };
        open.truncate(keep);
    }
    if left > 0 {
        if opens && (marker != '~' || left == 2) {
            open.push(Open {
                marker,
                len: left,
                at: end,
            });
        } else {
            *content = Some(end - 1);
        }
    }
}

/// Whether the last line is one or two dashes or equals signs under a line
/// that has text in it.
fn looks_like_underline(text: &str) -> bool {
    let Some(line) = text.rfind('\n') else {
        return false;
    };
    let last = &text[line + 1..];
    let trimmed = last.trim_start();
    let all = |char: char| {
        !trimmed.is_empty() && trimmed.len() <= 2 && trimmed.chars().all(|next| next == char)
    };
    (all('-') || all('='))
        && text[..line]
            .lines()
            .next_back()
            .is_some_and(|above| !above.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::markdown::parse::{Block, Document, Inline};

    /// What a mended source reads as, so a test can say what the reader sees
    /// rather than what the string became.
    fn styled(text: &str) -> Vec<String> {
        let mended = close_hanging(text).unwrap_or_else(|| text.to_string());
        let document = Document::parse(&mended);
        let mut styled = Vec::new();
        for block in &document.blocks {
            if let Block::Paragraph(inlines) = block {
                collect(inlines, &mut styled);
            }
        }
        styled
    }

    fn collect(inlines: &[Inline], into: &mut Vec<String>) {
        for inline in inlines {
            match inline {
                Inline::Strong(children) => {
                    into.push(format!("strong:{}", flat(children)));
                }
                Inline::Emphasis(children) => {
                    into.push(format!("emphasis:{}", flat(children)));
                }
                Inline::Struck(children) => {
                    into.push(format!("struck:{}", flat(children)));
                }
                Inline::Code(text) => into.push(format!("code:{text}")),
                Inline::Link { href, content, .. } => {
                    into.push(format!("link:{href}:{}", flat(content)));
                }
                _ => {}
            }
        }
    }

    fn flat(inlines: &[Inline]) -> String {
        inlines
            .iter()
            .map(|inline| match inline {
                Inline::Text(text) => text.to_string(),
                Inline::Code(text) => text.to_string(),
                Inline::Strong(children)
                | Inline::Emphasis(children)
                | Inline::Struck(children) => flat(children),
                _ => String::new(),
            })
            .collect()
    }

    #[test]
    fn nothing_hanging_is_left_alone() {
        // The common case has to cost nothing and change nothing.
        assert_eq!(close_hanging("Plain text with **bold** closed."), None);
        assert_eq!(close_hanging(""), None);
        assert_eq!(close_hanging("A `span` and a [link](/there)."), None);
    }

    #[test]
    fn an_opener_with_content_after_it_is_styled_immediately() {
        assert_eq!(styled("This is **bold"), vec!["strong:bold"]);
        assert_eq!(styled("This is *quiet"), vec!["emphasis:quiet"]);
        assert_eq!(styled("This is ~~gone"), vec!["struck:gone"]);
        assert_eq!(styled("This is `code"), vec!["code:code"]);
    }

    #[test]
    fn an_opener_with_nothing_after_it_stays_literal() {
        // There is nothing to style yet, and styling emptiness would put a
        // mark on the screen that the source does not contain.
        assert_eq!(close_hanging("This is **"), None);
        assert_eq!(close_hanging("This is ** "), None);
    }

    #[test]
    fn nesting_closes_from_the_inside_out() {
        assert_eq!(styled("**bold and *quiet"), vec!["strong:bold and quiet"]);
    }

    #[test]
    fn a_closer_that_is_itself_half_arrived_still_closes() {
        // `**a*` is one asterisk short of finished. Reading it as emphasis
        // inside nothing, then restyling, is the flicker being avoided.
        assert_eq!(styled("**bold*"), vec!["strong:bold"]);
    }

    #[test]
    fn a_link_whose_address_is_still_arriving_shows_its_words() {
        // The words are what the reader is reading. The address is not shown
        // either way, so it can wait, and it must not be followed meanwhile.
        assert_eq!(
            styled("See [the notes](https://exa"),
            vec![format!("link:{PENDING_LINK}:the notes")]
        );
        assert_eq!(
            styled("See [the notes"),
            vec![format!("link:{PENDING_LINK}:the notes")]
        );
    }

    #[test]
    fn a_dash_beginning_a_list_is_not_read_as_an_underline() {
        // Without this the paragraph above flashes into a heading for as long
        // as it takes the rest of the list item to arrive.
        let mended = close_hanging("Some text\n-").expect("the ambiguity is repaired");
        let document = Document::parse(&mended);
        assert!(
            !document
                .blocks
                .iter()
                .any(|block| matches!(block, Block::Heading { .. })),
            "a list item that has just started is not a heading"
        );
    }

    #[test]
    fn arithmetic_is_not_emphasis() {
        assert_eq!(close_hanging("2*3"), None);
        assert_eq!(close_hanging("snake_case_name"), None);
    }

    #[test]
    fn an_escaped_marker_does_not_hang() {
        assert_eq!(close_hanging("a \\*not emphasis"), None);
    }

    #[test]
    fn a_marker_inside_a_finished_code_span_is_not_a_marker() {
        assert_eq!(close_hanging("`a ** b`"), None);
    }
}
