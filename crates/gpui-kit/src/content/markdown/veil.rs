//! Letting arriving text appear rather than snap into place.
//!
//! Text committed to layout the instant it arrives is the right decision:
//! holding it back to animate it means a reader waiting on a machine that has
//! already answered. But characters appearing at full strength, a few at a
//! time, read as a stutter — the eye is drawn to each arrival instead of to
//! the sentence.
//!
//! A veil fixes that without delaying anything. The text is laid out
//! immediately and at its final position; what changes is only its opacity,
//! for a fraction of a second, over the characters that just landed. Several
//! chunks fade at once when a stream is fast, so the effect is a soft leading
//! edge rather than a blink per token, and a chunk fades exactly once —
//! settled text never breathes again.
//!
//! Nothing here moves anything. Opacity is the whole vocabulary, because a
//! positional offset on text that has already been committed would mean the
//! reader's eye tracking a word that is not going anywhere.
//!
//! The fade's length follows the stream's own cadence: an average of the gaps
//! between arrivals, so a slow deliberate answer fades gently and a burst does
//! not queue up half a second of backlog behind itself.

use std::ops::Range;

use web_time::Instant;

/// Where the cadence estimate starts, before there is a cadence.
const SEED_MS: f32 = 160.0;
/// The shortest a fade may be. Below this it is a flash, not a fade.
const MIN_FADE_MS: f32 = 120.0;
/// The longest a fade may be. Beyond this the text reads as broken rather than
/// arriving.
const MAX_FADE_MS: f32 = 400.0;
/// How the veil dissolves. Above one, so it clears quickly and then lingers
/// faintly, which reads as the text settling rather than as a linear wipe.
const CURVE: f32 = 1.6;
/// The longest gap that says anything about cadence. A stream that paused for
/// a minute has not become a slow stream.
const GAP_CLAMP_MS: f32 = 1000.0;
/// Past this many chunks in flight the stream is outrunning the fade, so the
/// fade gets out of its way.
const BACKLOG: usize = 3;
/// How much faster a backed-up fade runs.
const BACKLOG_SPEEDUP: f32 = 0.75;

/// One arrival, mid-fade.
#[derive(Debug, Clone)]
struct Chunk {
    /// Which of the document's text runs this landed in.
    run: usize,
    /// Where in that run, in bytes.
    range: Range<usize>,
    started: Instant,
    /// Fixed when it arrived, so a chunk's fade is not re-timed by what
    /// arrives after it.
    duration_ms: f32,
}

/// The fade over one document's most recent arrivals.
#[derive(Debug, Default)]
pub(crate) struct Veil {
    /// The text runs as the last frame drew them, in drawing order.
    ///
    /// Recorded by the painter rather than derived from the tree, so what the
    /// veil believes is on screen is what was actually put there.
    runs: Vec<String>,
    chunks: Vec<Chunk>,
    /// The average gap between arrivals.
    cadence: f32,
    last: Option<Instant>,
}

impl Veil {
    /// Takes in what the frame drew, and works out what of it is new.
    ///
    /// Only growth is treated as arrival: text appended to the last run, or
    /// runs added after the ones already there. Any other difference is a
    /// document that changed rather than one that grew — an edit, a retry, a
    /// different answer — and that appears settled, because fading it would be
    /// claiming it just arrived when it did not.
    pub(crate) fn observe(&mut self, runs: Vec<String>, now: Instant) {
        if self.runs.is_empty() {
            // The first sight of a document is not an arrival. Fading it in
            // would animate opening a conversation, not receiving one.
            self.runs = runs;
            return;
        }
        let grown = runs.len() >= self.runs.len()
            && self
                .runs
                .iter()
                .zip(&runs)
                .enumerate()
                .all(|(index, (before, after))| {
                    if index + 1 == self.runs.len() {
                        after.starts_with(before.as_str())
                    } else {
                        before == after
                    }
                });
        if !grown {
            self.chunks.clear();
            self.runs = runs;
            return;
        }

        let gap = self
            .last
            .map(|last| {
                (now.saturating_duration_since(last).as_secs_f32() * 1000.0).min(GAP_CLAMP_MS)
            })
            .unwrap_or(SEED_MS);
        self.cadence = if self.cadence == 0.0 {
            gap
        } else {
            self.cadence + 0.3 * (gap - self.cadence)
        };
        self.last = Some(now);

        // Three times the cadence, so a chunk is still fading while the next
        // few arrive: one soft edge rather than a row of separate blinks.
        let mut duration = (self.cadence * 3.0).clamp(MIN_FADE_MS, MAX_FADE_MS);
        self.drop_finished(now);
        if self.chunks.len() >= BACKLOG {
            duration *= BACKLOG_SPEEDUP;
        }

        let mut arrived = false;
        for (index, after) in runs.iter().enumerate() {
            let before = self.runs.get(index).map_or(0, String::len);
            if after.len() > before {
                self.chunks.push(Chunk {
                    run: index,
                    range: before..after.len(),
                    started: now,
                    duration_ms: duration,
                });
                arrived = true;
            }
        }
        if !arrived {
            self.last = None;
        }
        self.runs = runs;
    }

    /// How opaque each part of run `index` should be drawn, where it is not
    /// yet fully opaque.
    ///
    /// An empty answer means "draw it normally", which is the answer for
    /// almost every run of almost every document.
    pub(crate) fn spans(&self, index: usize, now: Instant) -> Vec<(Range<usize>, f32)> {
        self.chunks
            .iter()
            .filter(|chunk| chunk.run == index)
            .filter_map(|chunk| {
                let elapsed = now.saturating_duration_since(chunk.started).as_secs_f32() * 1000.0;
                let progress = (elapsed / chunk.duration_ms).clamp(0.0, 1.0);
                (progress < 1.0).then(|| (chunk.range.clone(), opacity(progress)))
            })
            .collect()
    }

    /// Whether anything is still fading, and so whether another frame is owed.
    pub(crate) fn is_fading(&self, now: Instant) -> bool {
        self.chunks.iter().any(|chunk| {
            now.saturating_duration_since(chunk.started).as_secs_f32() * 1000.0 < chunk.duration_ms
        })
    }

    fn drop_finished(&mut self, now: Instant) {
        self.chunks.retain(|chunk| {
            now.saturating_duration_since(chunk.started).as_secs_f32() * 1000.0 < chunk.duration_ms
        });
    }
}

/// How much of the text shows through at `progress` through its fade.
fn opacity(progress: f32) -> f32 {
    1.0 - (1.0 - progress).powf(CURVE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn runs(texts: &[&str]) -> Vec<String> {
        texts.iter().map(|text| (*text).to_string()).collect()
    }

    #[test]
    fn the_first_sight_of_a_document_is_not_an_arrival() {
        // Opening a conversation is not the same as receiving one, and fading
        // in what was already there would animate the wrong event.
        let mut veil = Veil::default();
        let now = Instant::now();
        veil.observe(runs(&["Already written."]), now);
        assert!(veil.spans(0, now).is_empty());
        assert!(!veil.is_fading(now));
    }

    #[test]
    fn text_appended_to_a_run_fades_from_where_it_starts() {
        let mut veil = Veil::default();
        let start = Instant::now();
        veil.observe(runs(&["Hello"]), start);
        veil.observe(runs(&["Hello there"]), start);

        let spans = veil.spans(0, start);
        assert_eq!(spans.len(), 1);
        let (range, opacity) = &spans[0];
        assert_eq!(*range, 5..11, "only the characters that just landed");
        assert!(*opacity < 0.01, "a fade starts from nothing");
    }

    #[test]
    fn a_fade_finishes_and_never_starts_again() {
        let mut veil = Veil::default();
        let start = Instant::now();
        veil.observe(runs(&["Hello"]), start);
        veil.observe(runs(&["Hello there"]), start);
        assert!(veil.is_fading(start));

        let later = start + Duration::from_millis(MAX_FADE_MS as u64 + 1);
        assert!(veil.spans(0, later).is_empty(), "the fade is over");
        assert!(!veil.is_fading(later));

        // Settled text must not breathe when the next chunk arrives.
        veil.observe(runs(&["Hello there, friend"]), later);
        let spans = veil.spans(0, later);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].0, 11..19, "only the newest characters fade");
    }

    #[test]
    fn several_arrivals_fade_at_once() {
        // A fast stream should read as one soft leading edge, which means
        // chunks overlapping rather than queueing.
        let mut veil = Veil::default();
        let start = Instant::now();
        veil.observe(runs(&["a"]), start);
        veil.observe(runs(&["ab"]), start + Duration::from_millis(10));
        veil.observe(runs(&["abc"]), start + Duration::from_millis(20));
        veil.observe(runs(&["abcd"]), start + Duration::from_millis(30));

        let spans = veil.spans(0, start + Duration::from_millis(30));
        assert!(spans.len() > 1, "a fast stream keeps several chunks alive");
    }

    #[test]
    fn a_document_that_changed_rather_than_grew_appears_settled() {
        // A retry, an edit, a different answer under the same identity. None
        // of it just arrived, so none of it fades.
        let mut veil = Veil::default();
        let now = Instant::now();
        veil.observe(runs(&["First answer"]), now);
        veil.observe(runs(&["First answer more"]), now);
        assert!(veil.is_fading(now));

        veil.observe(runs(&["A completely different answer"]), now);
        assert!(
            veil.spans(0, now).is_empty(),
            "replacing a document is not receiving one"
        );
    }

    #[test]
    fn a_new_run_after_the_existing_ones_fades_whole() {
        let mut veil = Veil::default();
        let now = Instant::now();
        veil.observe(runs(&["First"]), now);
        veil.observe(runs(&["First", "Second"]), now);
        assert_eq!(
            veil.spans(1, now).first().map(|(range, _)| range.clone()),
            Some(0..6)
        );
    }

    #[test]
    fn opacity_climbs_from_nothing_to_everything() {
        assert!(opacity(0.0) < 0.001);
        assert!((opacity(1.0) - 1.0).abs() < 0.001);
        assert!(opacity(0.5) > 0.5, "it clears early and lingers faintly");
        // Monotone, or the text would brighten and dim on its way in.
        let mut previous = 0.0;
        for step in 0..=20 {
            let value = opacity(step as f32 / 20.0);
            assert!(value >= previous - 1e-6);
            previous = value;
        }
    }
}
