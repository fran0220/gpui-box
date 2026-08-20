//! One selection that spans separately mounted text elements.
//!
//! [`StyledText::selectable`](crate::StyledText::selectable) gives a single
//! shaped value its own selection. That is the right primitive for one label,
//! and the wrong one for a document: a reader dragging through prose does not
//! know that each paragraph was mounted separately, and neither should the
//! gesture.
//!
//! A participant states its identity, the scope it belongs to, and where it
//! sits in reading order, once per frame:
//!
//! ```
//! # use gpui::{Bounds, Pixels, SelectionContentKey, SelectionParticipant};
//! # fn example(bounds: Bounds<Pixels>) -> SelectionParticipant {
//! SelectionParticipant::new(SelectionContentKey::new("note.42.body"), 42, bounds)
//!     .text("the paragraph as it reads")
//! # }
//! ```
//!
//! Three facts hold, and each is what makes the coordinator safe to point at
//! product data:
//!
//! - **Identity is business identity.** A key derives from what the content
//!   *is*, never from where it landed in a list, so a selection survives
//!   reordering, insertion, and re-virtualization.
//! - **A scope cannot be selected across.** A dialog mounted over a document
//!   pushes its own [`SelectionScopeId`]; a drag inside it never reaches the
//!   text behind it, and dismissing it leaves nothing behind.
//! - **Sensitive content never registers.** A participant marked
//!   [`SelectionParticipant::sensitive`] is refused at registration, so no
//!   credential can reach the aggregate copy path by being mounted next to
//!   something that can.
//!
//! # Reading order is declared, not inferred
//!
//! Paint order is a rendering detail and map order is arbitrary, so neither
//! can decide what "between" means. Each participant declares a `u64` order,
//! and both endpoints of a selection retain the order they were placed at.
//! Resolution therefore never depends on the participant at the other end
//! still being mounted.
//!
//! # What a copy can and cannot prove
//!
//! [`DocumentSelectionState::copy`] walks the participants registered for the
//! current frame in reading order. It reports
//! [`SelectionCopy::complete`] as `false` when it cannot prove it saw the
//! whole span — either because an endpoint is no longer mounted, or because
//! the span crosses a participant that declared itself
//! [`SelectionParticipant::virtualized`] and therefore vouches only for
//! itself. A host that needs the whole document supplies it; GPUI does not
//! invent the rows it never rendered.

use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::rc::Rc;

use collections::FxHashMap;

use crate::{Bounds, Pixels, Point, SharedString};

/// The stable identity of one participant in a document selection.
///
/// A key states what the content *is*. Deriving one from a row index makes a
/// selection follow the viewport instead of the text, which is the failure this
/// type exists to prevent.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SelectionContentKey(SharedString);

impl SelectionContentKey {
    /// Names one participant by its business identity.
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self(id.into())
    }

    /// The identity as it was declared.
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

impl From<SharedString> for SelectionContentKey {
    fn from(value: SharedString) -> Self {
        Self(value)
    }
}

impl From<&'static str> for SelectionContentKey {
    fn from(value: &'static str) -> Self {
        Self(SharedString::from(value))
    }
}

impl From<String> for SelectionContentKey {
    fn from(value: String) -> Self {
        Self(SharedString::from(value))
    }
}

/// An opaque isolation boundary within one window.
///
/// Two participants in different scopes are never part of the same selection,
/// whatever their reading order says.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SelectionScopeId(u64);

impl SelectionScopeId {
    /// The scope a participant belongs to when nothing pushed another one.
    pub const ROOT: Self = Self(0);

    /// Derives a stable scope from anything a caller already identifies an
    /// isolated surface by, such as a dialog's element id.
    pub fn new(seed: impl Hash) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        seed.hash(&mut hasher);
        // Zero is reserved for the root scope, so a hash that lands there is
        // moved rather than silently merging a modal into the document.
        Self(hasher.finish() | 1)
    }
}

/// How much of its content a participant can vouch for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionCoverage {
    /// Every participant of this content is mounted whenever any of it is, so
    /// a span that crosses it has been seen in full.
    #[default]
    Complete,
    /// This participant is one mounted window onto a longer run. It can report
    /// its own text and nothing about its neighbours.
    Virtualized,
}

/// Maps a window position to a byte offset in one participant's text.
///
/// The participant that owns a drag needs this to place the far end of a
/// selection inside a participant it has no layout for.
pub type SelectionResolver = Rc<dyn Fn(Point<Pixels>) -> usize>;

/// What one participant reports about itself for one frame.
#[derive(Clone)]
pub struct SelectionParticipant {
    key: SelectionContentKey,
    scope: SelectionScopeId,
    order: u64,
    bounds: Bounds<Pixels>,
    text: SharedString,
    rows: Vec<Range<usize>>,
    coverage: SelectionCoverage,
    sensitive: bool,
    resolve: Option<SelectionResolver>,
}

impl std::fmt::Debug for SelectionParticipant {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SelectionParticipant")
            .field("key", &self.key)
            .field("scope", &self.scope)
            .field("order", &self.order)
            .field("bounds", &self.bounds)
            .field("coverage", &self.coverage)
            .field("sensitive", &self.sensitive)
            .finish_non_exhaustive()
    }
}

impl SelectionParticipant {
    /// Declares a participant at a position in reading order.
    pub fn new(key: impl Into<SelectionContentKey>, order: u64, bounds: Bounds<Pixels>) -> Self {
        Self {
            key: key.into(),
            scope: SelectionScopeId::ROOT,
            order,
            bounds,
            text: SharedString::default(),
            rows: Vec::new(),
            coverage: SelectionCoverage::Complete,
            sensitive: false,
            resolve: None,
        }
    }

    /// Supplies the mapping from a window position to a byte offset.
    ///
    /// Without one, a drag reaching this participant lands at its start or its
    /// end rather than inside it.
    pub fn resolver(mut self, resolve: SelectionResolver) -> Self {
        self.resolve = Some(resolve);
        self
    }

    /// The text this participant projects selection offsets onto.
    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.text = text.into();
        self
    }

    /// The visual rows a triple click selects by.
    pub fn rows(mut self, rows: Vec<Range<usize>>) -> Self {
        self.rows = rows;
        self
    }

    /// Places the participant in an isolated scope.
    pub fn scope(mut self, scope: SelectionScopeId) -> Self {
        self.scope = scope;
        self
    }

    /// States that this participant is one mounted window onto a longer run,
    /// so a span crossing it cannot be reported as a complete copy.
    pub fn virtualized(mut self) -> Self {
        self.coverage = SelectionCoverage::Virtualized;
        self
    }

    /// Refuses registration entirely.
    ///
    /// A sensitive participant takes no part in a document selection and its
    /// bytes never reach the aggregate copy path.
    pub fn sensitive(mut self, sensitive: bool) -> Self {
        self.sensitive = sensitive;
        self
    }

    /// Whether this participant declined to take part.
    pub fn is_sensitive(&self) -> bool {
        self.sensitive
    }

    /// The identity this participant declared.
    pub fn key(&self) -> &SelectionContentKey {
        &self.key
    }
}

/// A participant as it was registered for the frame being drawn.
#[derive(Clone)]
pub(crate) struct RegisteredParticipant {
    pub(crate) scope: SelectionScopeId,
    pub(crate) order: u64,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) text: SharedString,
    pub(crate) rows: Vec<Range<usize>>,
    pub(crate) coverage: SelectionCoverage,
    pub(crate) resolve: Option<SelectionResolver>,
}

impl RegisteredParticipant {
    fn from_declared(participant: &SelectionParticipant) -> Self {
        Self {
            scope: participant.scope,
            order: participant.order,
            bounds: participant.bounds,
            text: participant.text.clone(),
            rows: participant.rows.clone(),
            coverage: participant.coverage,
            resolve: participant.resolve.clone(),
        }
    }
}

impl RegisteredParticipant {
    /// The byte offset a window position names within this participant.
    pub(crate) fn offset_at(&self, position: Point<Pixels>) -> usize {
        match self.resolve.as_ref() {
            Some(resolve) => resolve(position).min(self.text.len()),
            None => self.text.len(),
        }
    }
}

/// One end of a document selection.
///
/// The endpoint retains the reading order it was placed at, so resolving a
/// range never requires the participant at the other end to still be mounted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectionEndpoint {
    /// Which participant the end sits in.
    pub key: SelectionContentKey,
    /// That participant's reading order when the end was placed.
    pub order: u64,
    /// The byte offset within that participant's text.
    pub offset: usize,
}

impl SelectionEndpoint {
    fn sort_key(&self) -> (u64, usize, &str) {
        (self.order, self.offset, self.key.as_str())
    }
}

/// The result of copying a document selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectionCopy {
    /// The selected text of every participant the copy could see, joined in
    /// reading order.
    pub text: String,
    /// How many participants contributed.
    pub participants: usize,
    /// Whether the copy could prove it saw the whole span.
    ///
    /// `false` means content was selected that GPUI could not read: an
    /// endpoint is no longer mounted, or the span crosses a participant that
    /// declared itself [`SelectionCoverage::Virtualized`]. The text is still
    /// what was actually read, never a guess.
    pub complete: bool,
}

/// Which selection gesture a pointer press opened.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionUnit {
    /// One insertion point, extended by graphemes.
    #[default]
    Caret,
    /// A word, extended a word at a time.
    Word,
    /// A visual row, extended a row at a time.
    Row,
}

/// The document selection of one window.
///
/// This lives on [`Window`](crate::Window) rather than in a global, so it
/// follows the window's own lifetime and a closed window leaves nothing
/// behind.
#[derive(Clone, Debug, Default)]
pub struct DocumentSelectionState {
    scope: SelectionScopeId,
    anchor: Option<SelectionEndpoint>,
    focus: Option<SelectionEndpoint>,
    dragging: bool,
    drag_origin: Option<SelectionContentKey>,
    drag_unit: Option<(SelectionContentKey, u64, Range<usize>)>,
    drag_kind: SelectionUnit,
    autoscroll: Point<Pixels>,
}

impl DocumentSelectionState {
    /// The scope the current selection belongs to.
    pub fn scope(&self) -> SelectionScopeId {
        self.scope
    }

    /// The end the selection was started from.
    pub fn anchor(&self) -> Option<&SelectionEndpoint> {
        self.anchor.as_ref()
    }

    /// The end the selection currently reaches.
    pub fn focus(&self) -> Option<&SelectionEndpoint> {
        self.focus.as_ref()
    }

    /// Whether a pointer drag is extending the selection right now.
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// Whether anything at all is selected.
    pub fn is_empty(&self) -> bool {
        match (&self.anchor, &self.focus) {
            (Some(anchor), Some(focus)) => anchor == focus,
            _ => true,
        }
    }

    /// How far a host should scroll to keep a drag that left the mounted
    /// content moving, in logical pixels per frame.
    ///
    /// Zero whenever no drag is in progress. GPUI reports the overflow and
    /// scrolls nothing itself, because it does not own the container.
    pub fn autoscroll_delta(&self) -> Point<Pixels> {
        if self.dragging {
            self.autoscroll
        } else {
            Point::default()
        }
    }

    /// Forgets the selection entirely.
    pub fn clear(&mut self) {
        self.scope = SelectionScopeId::ROOT;
        self.anchor = None;
        self.focus = None;
        self.dragging = false;
        self.drag_origin = None;
        self.drag_unit = None;
        self.drag_kind = SelectionUnit::Caret;
        self.autoscroll = Point::default();
    }

    /// The unit a drag in progress extends by.
    pub(crate) fn drag_kind(&self) -> SelectionUnit {
        self.drag_kind
    }

    /// Whether `key` is the participant that owns the drag in progress.
    ///
    /// Exactly one participant answers a pointer move, so a drag that has left
    /// the text it started in still has one owner rather than as many opinions
    /// as there are mounted paragraphs.
    pub fn owns_drag(&self, key: &SelectionContentKey) -> bool {
        self.dragging && self.drag_origin.as_ref() == Some(key)
    }

    /// The two ends in reading order, low first.
    pub fn ordered(&self) -> Option<(&SelectionEndpoint, &SelectionEndpoint)> {
        let (anchor, focus) = (self.anchor.as_ref()?, self.focus.as_ref()?);
        if anchor.sort_key() <= focus.sort_key() {
            Some((anchor, focus))
        } else {
            Some((focus, anchor))
        }
    }

    /// The byte range of one participant that this selection covers.
    ///
    /// Returns `None` when the participant lies outside the selection or in
    /// another scope. A participant strictly between the two ends is covered
    /// in full without needing to have been registered when the ends were
    /// placed.
    pub fn range_for(
        &self,
        key: &SelectionContentKey,
        scope: SelectionScopeId,
        order: u64,
        text_len: usize,
    ) -> Option<Range<usize>> {
        if scope != self.scope {
            return None;
        }
        let (start, end) = self.ordered()?;
        let here = (order, key.as_str());
        let low = (start.order, start.key.as_str());
        let high = (end.order, end.key.as_str());
        if here < low || here > high {
            return None;
        }
        let from = if here == low {
            start.offset.min(text_len)
        } else {
            0
        };
        let to = if here == high {
            end.offset.min(text_len)
        } else {
            text_len
        };
        (from <= to).then_some(from..to)
    }

    pub(crate) fn begin(
        &mut self,
        scope: SelectionScopeId,
        endpoint: SelectionEndpoint,
        unit: Range<usize>,
        kind: SelectionUnit,
    ) {
        if self.scope != scope {
            self.clear();
        }
        self.scope = scope;
        let order = endpoint.order;
        let key = endpoint.key.clone();
        let key_for_origin = key.clone();
        self.anchor = Some(SelectionEndpoint {
            key: key.clone(),
            order,
            offset: unit.start,
        });
        self.focus = Some(SelectionEndpoint {
            key: key.clone(),
            order,
            offset: unit.end,
        });
        self.drag_unit = match kind {
            SelectionUnit::Caret => None,
            _ => Some((key, order, unit)),
        };
        self.dragging = true;
        self.drag_origin = Some(key_for_origin);
        self.drag_kind = kind;
        self.autoscroll = Point::default();
    }

    pub(crate) fn extend_to(&mut self, scope: SelectionScopeId, endpoint: SelectionEndpoint) {
        self.extend_to_snapped(scope, endpoint, None);
    }

    /// Moves the far end, snapping it outward to `target_unit` when the drag
    /// opened on a word or a row.
    ///
    /// `target_unit` is the word or row containing the endpoint *in the
    /// participant the endpoint fell in*, which is why the caller supplies it:
    /// a drag that crossed into another paragraph must snap to that
    /// paragraph's boundaries, not to the ones it started from.
    pub(crate) fn extend_to_snapped(
        &mut self,
        scope: SelectionScopeId,
        endpoint: SelectionEndpoint,
        target_unit: Option<Range<usize>>,
    ) {
        if self.scope != scope || self.anchor.is_none() {
            return;
        }
        match self.drag_unit.clone() {
            Some((unit_key, unit_order, unit)) => {
                let here = (endpoint.order, endpoint.offset);
                if here < (unit_order, unit.start) {
                    let offset = target_unit
                        .as_ref()
                        .map_or(endpoint.offset, |target| target.start);
                    let endpoint = SelectionEndpoint { offset, ..endpoint };
                    self.anchor = Some(SelectionEndpoint {
                        key: unit_key,
                        order: unit_order,
                        offset: unit.end,
                    });
                    self.focus = Some(endpoint);
                } else if here > (unit_order, unit.end) {
                    let offset = target_unit
                        .as_ref()
                        .map_or(endpoint.offset, |target| target.end);
                    let endpoint = SelectionEndpoint { offset, ..endpoint };
                    self.anchor = Some(SelectionEndpoint {
                        key: unit_key.clone(),
                        order: unit_order,
                        offset: unit.start,
                    });
                    self.focus = Some(endpoint);
                } else {
                    self.anchor = Some(SelectionEndpoint {
                        key: unit_key.clone(),
                        order: unit_order,
                        offset: unit.start,
                    });
                    self.focus = Some(SelectionEndpoint {
                        key: unit_key,
                        order: unit_order,
                        offset: unit.end,
                    });
                }
            }
            None => self.focus = Some(endpoint),
        }
    }

    /// Starts a Shift-extension from the existing anchor.
    ///
    /// A Shift press with nothing selected yet is an ordinary caret press, so
    /// the gesture never depends on an anchor that is not there.
    pub(crate) fn begin_shift_extend(
        &mut self,
        scope: SelectionScopeId,
        endpoint: SelectionEndpoint,
    ) {
        if self.scope != scope || self.anchor.is_none() {
            let offset = endpoint.offset;
            self.begin(scope, endpoint, offset..offset, SelectionUnit::Caret);
            return;
        }
        self.drag_unit = None;
        self.drag_kind = SelectionUnit::Caret;
        self.drag_origin = Some(endpoint.key.clone());
        self.dragging = true;
        self.focus = Some(endpoint);
    }

    pub(crate) fn end_drag(&mut self) {
        self.dragging = false;
        self.drag_origin = None;
        self.drag_unit = None;
        self.autoscroll = Point::default();
    }

    pub(crate) fn set_autoscroll(&mut self, delta: Point<Pixels>) {
        self.autoscroll = delta;
    }

    pub(crate) fn select_all(&mut self, scope: SelectionScopeId, participants: &[Registered]) {
        let mut within = participants
            .iter()
            .filter(|participant| participant.entry.scope == scope)
            .collect::<Vec<_>>();
        within.sort_by(|left, right| {
            (left.entry.order, left.key.as_str()).cmp(&(right.entry.order, right.key.as_str()))
        });
        let (Some(first), Some(last)) = (within.first(), within.last()) else {
            return;
        };
        self.scope = scope;
        self.anchor = Some(SelectionEndpoint {
            key: first.key.clone(),
            order: first.entry.order,
            offset: 0,
        });
        self.focus = Some(SelectionEndpoint {
            key: last.key.clone(),
            order: last.entry.order,
            offset: last.entry.text.len(),
        });
        self.drag_unit = None;
        self.dragging = false;
    }

    /// Reads the selection out of the participants registered for this frame.
    pub(crate) fn copy(&self, participants: &[Registered]) -> Option<SelectionCopy> {
        let (start, end) = self.ordered()?;
        if start == end {
            return None;
        }
        let mut within = participants
            .iter()
            .filter(|participant| participant.entry.scope == self.scope)
            .collect::<Vec<_>>();
        within.sort_by(|left, right| {
            (left.entry.order, left.key.as_str()).cmp(&(right.entry.order, right.key.as_str()))
        });

        let mut text = String::new();
        let mut contributed = 0usize;
        let mut crosses_virtualized = false;
        let mut saw_start = false;
        let mut saw_end = false;
        for participant in &within {
            if participant.key == start.key {
                saw_start = true;
            }
            if participant.key == end.key {
                saw_end = true;
            }
            let Some(range) = self.range_for(
                &participant.key,
                participant.entry.scope,
                participant.entry.order,
                participant.entry.text.len(),
            ) else {
                continue;
            };
            if participant.entry.coverage == SelectionCoverage::Virtualized {
                crosses_virtualized = true;
            }
            if range.is_empty() {
                continue;
            }
            if contributed > 0 {
                text.push('\n');
            }
            text.push_str(&participant.entry.text[range]);
            contributed += 1;
        }

        let spans_many = start.key != end.key;
        let complete = saw_start && saw_end && (!spans_many || !crosses_virtualized);
        (contributed > 0).then_some(SelectionCopy {
            text,
            participants: contributed,
            complete,
        })
    }
}

/// A registered participant paired with the key it registered under.
#[derive(Clone)]
pub(crate) struct Registered {
    pub(crate) key: SelectionContentKey,
    pub(crate) entry: RegisteredParticipant,
}

/// Where a pointer position lands among the participants of one scope.
pub(crate) enum Landing {
    /// Inside a participant, at a byte offset the caller resolves.
    Inside(Registered),
    /// Past the end of the last participant above the pointer.
    After(Registered),
    /// Before the start of the first participant below the pointer.
    Before(Registered),
    /// No participant in this scope is mounted.
    Nowhere,
}

/// Resolves a pointer position against the participants of one scope.
///
/// A drag that leaves the text it started in still has to mean something, so a
/// position between two participants extends to whichever edge it passed.
impl Landing {
    /// A word for the case this is, used when a test has to say which one it
    /// got instead.
    #[cfg(test)]
    fn name(&self) -> &'static str {
        match self {
            Self::Inside(_) => "inside",
            Self::After(_) => "after",
            Self::Before(_) => "before",
            Self::Nowhere => "nowhere",
        }
    }
}

pub(crate) fn land_selection(
    participants: &[Registered],
    scope: SelectionScopeId,
    position: Point<Pixels>,
) -> Landing {
    let mut within = participants
        .iter()
        .filter(|participant| participant.entry.scope == scope)
        .collect::<Vec<_>>();
    within.sort_by(|left, right| {
        (left.entry.order, left.key.as_str()).cmp(&(right.entry.order, right.key.as_str()))
    });
    if within.is_empty() {
        return Landing::Nowhere;
    }
    // A position over a participant belongs to that participant, whichever
    // way its neighbours are arranged. Resolving by row alone would hand every
    // press on a line of inline runs to the leftmost of them, which is how a
    // click on a link came back as a selection reaching the start of its
    // paragraph.
    if let Some(participant) = within
        .iter()
        .find(|participant| participant.entry.bounds.contains(&position))
    {
        return Landing::Inside((*participant).clone());
    }

    // The same row, but in a gap between runs or in a margin beside them. The
    // last run the position is past is the one it passed; before all of them
    // it is the leading edge of the row.
    let on_row = within
        .iter()
        .filter(|participant| {
            let bounds = participant.entry.bounds;
            position.y >= bounds.top() && position.y < bounds.bottom()
        })
        .collect::<Vec<_>>();
    if !on_row.is_empty() {
        let passed = on_row
            .iter()
            .rfind(|participant| participant.entry.bounds.right() <= position.x);
        return match passed {
            Some(participant) => Landing::After((**participant).clone()),
            None => Landing::Before((*on_row[0]).clone()),
        };
    }

    let above = within
        .iter()
        .rfind(|participant| participant.entry.bounds.bottom() <= position.y);
    if let Some(participant) = above {
        return Landing::After((*participant).clone());
    }
    let below = within
        .iter()
        .find(|participant| participant.entry.bounds.top() > position.y);
    match below {
        Some(participant) => Landing::Before((*participant).clone()),
        None => Landing::Nowhere,
    }
}

/// The vertical overflow of a drag past the mounted content of one scope.
/// The word or visual row of `key` that contains `offset`.
///
/// Returns `None` for a caret drag, or when the participant is not mounted.
pub(crate) fn selection_unit_range(
    participants: &[Registered],
    key: &SelectionContentKey,
    offset: usize,
    kind: SelectionUnit,
) -> Option<Range<usize>> {
    if kind == SelectionUnit::Caret {
        return None;
    }
    let target = participants.iter().find(|entry| entry.key == *key)?;
    Some(match kind {
        SelectionUnit::Word => crate::word_range_at(&target.entry.text, offset),
        SelectionUnit::Row => crate::visual_row_at(&target.entry.rows, offset),
        SelectionUnit::Caret => return None,
    })
}

/// The endpoint a window position names within one scope.
pub(crate) fn selection_endpoint_at(
    participants: &[Registered],
    scope: SelectionScopeId,
    position: Point<Pixels>,
) -> Option<SelectionEndpoint> {
    match land_selection(participants, scope, position) {
        Landing::Inside(participant) => Some(SelectionEndpoint {
            key: participant.key,
            order: participant.entry.order,
            offset: participant.entry.offset_at(position),
        }),
        Landing::After(participant) => Some(SelectionEndpoint {
            key: participant.key,
            order: participant.entry.order,
            offset: participant.entry.text.len(),
        }),
        Landing::Before(participant) => Some(SelectionEndpoint {
            key: participant.key,
            order: participant.entry.order,
            offset: 0,
        }),
        Landing::Nowhere => None,
    }
}

pub(crate) fn selection_autoscroll_for(
    participants: &[Registered],
    scope: SelectionScopeId,
    position: Point<Pixels>,
) -> Point<Pixels> {
    let mut top = None::<Pixels>;
    let mut bottom = None::<Pixels>;
    for participant in participants
        .iter()
        .filter(|participant| participant.entry.scope == scope)
    {
        let bounds = participant.entry.bounds;
        top = Some(top.map_or(bounds.top(), |value: Pixels| value.min(bounds.top())));
        bottom = Some(bottom.map_or(bounds.bottom(), |value: Pixels| value.max(bounds.bottom())));
    }
    let (Some(top), Some(bottom)) = (top, bottom) else {
        return Point::default();
    };
    let delta = if position.y < top {
        position.y - top
    } else if position.y > bottom {
        position.y - bottom
    } else {
        Pixels::ZERO
    };
    Point::new(Pixels::ZERO, delta)
}

pub(crate) fn document_selection_register(
    map: &mut FxHashMap<(SelectionScopeId, SelectionContentKey), RegisteredParticipant>,
    participant: &SelectionParticipant,
) {
    if participant.sensitive {
        return;
    }
    map.insert(
        (participant.scope, participant.key.clone()),
        RegisteredParticipant::from_declared(participant),
    );
}

pub(crate) fn document_selection_registered(
    map: &FxHashMap<(SelectionScopeId, SelectionContentKey), RegisteredParticipant>,
) -> Vec<Registered> {
    map.iter()
        .map(|((_, key), entry)| Registered {
            key: key.clone(),
            entry: entry.clone(),
        })
        .collect()
}

/// Draws its child inside an isolated text-selection scope.
///
/// A drag started inside the scope never reaches text outside it, and text
/// outside never joins a selection started within, which is what keeps a
/// dialog mounted over a document from being selected together with it.
pub struct SelectionScope {
    scope: SelectionScopeId,
    child: Option<crate::AnyElement>,
}

/// Isolates `child` in its own text-selection scope, seeded by `seed`.
pub fn selection_scope(seed: impl Hash, child: impl crate::IntoElement) -> SelectionScope {
    SelectionScope {
        scope: SelectionScopeId::new(seed),
        child: Some(child.into_any_element()),
    }
}

impl crate::Element for SelectionScope {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<crate::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&crate::GlobalElementId>,
        _inspector_id: Option<&crate::InspectorElementId>,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) -> (crate::LayoutId, ()) {
        let scope = self.scope;
        let child = self
            .child
            .as_mut()
            .expect("required framework invariant must hold");
        let layout_id =
            window.with_selection_scope(scope, |window| child.request_layout(window, cx));
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&crate::GlobalElementId>,
        _inspector_id: Option<&crate::InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) {
        let scope = self.scope;
        let child = self
            .child
            .as_mut()
            .expect("required framework invariant must hold");
        window.with_selection_scope(scope, |window| child.prepaint(window, cx));
    }

    fn paint(
        &mut self,
        _id: Option<&crate::GlobalElementId>,
        _inspector_id: Option<&crate::InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) {
        let scope = self.scope;
        let child = self
            .child
            .as_mut()
            .expect("required framework invariant must hold");
        window.with_selection_scope(scope, |window| child.paint(window, cx));
    }
}

impl crate::IntoElement for SelectionScope {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{point, px, size};

    fn participant(key: &str, order: u64, text: &str, top: f32) -> Registered {
        Registered {
            key: SelectionContentKey::new(key.to_owned()),
            entry: RegisteredParticipant {
                scope: SelectionScopeId::ROOT,
                order,
                bounds: Bounds::new(point(px(0.), px(top)), size(px(100.), px(20.))),
                text: SharedString::from(text.to_owned()),
                rows: one_row(text),
                coverage: SelectionCoverage::Complete,
                resolve: None,
            },
        }
    }

    /// The single visual row of a one-line participant.
    fn one_row(text: &str) -> Vec<Range<usize>> {
        std::iter::once(0..text.len()).collect()
    }

    /// A selection already placed between two ends, without a gesture.
    fn span(anchor: SelectionEndpoint, focus: SelectionEndpoint) -> DocumentSelectionState {
        DocumentSelectionState {
            anchor: Some(anchor),
            focus: Some(focus),
            ..Default::default()
        }
    }

    fn endpoint(key: &str, order: u64, offset: usize) -> SelectionEndpoint {
        SelectionEndpoint {
            key: SelectionContentKey::new(key.to_owned()),
            order,
            offset,
        }
    }

    /// A participant at an explicit box, for the cases where two of them share
    /// a row and only their horizontal extent tells them apart.
    fn placed(key: &str, order: u64, text: &str, left: f32, top: f32, width: f32) -> Registered {
        let mut entry = participant(key, order, text, top);
        entry.entry.bounds = Bounds::new(point(px(left), px(top)), size(px(width), px(20.)));
        entry
    }

    #[test]
    fn a_press_over_a_run_belongs_to_that_run_not_to_its_row() {
        let participants = vec![
            placed("lead", 0, "a paragraph with ", 0., 0., 100.),
            placed("link", 1, "the run log", 100., 0., 60.),
            placed("tail", 2, " beside it", 160., 0., 50.),
        ];

        // Squarely over the middle run, which shares its row with two others.
        let landing = land_selection(
            &participants,
            SelectionScopeId::ROOT,
            point(px(120.), px(10.)),
        );
        match landing {
            Landing::Inside(participant) => assert_eq!(participant.key.as_str(), "link"),
            other => panic!("a press over a run landed elsewhere: {}", other.name()),
        }
    }

    #[test]
    fn a_press_in_a_gap_belongs_to_the_run_it_passed() {
        let participants = vec![
            placed("lead", 0, "a paragraph with ", 0., 0., 100.),
            placed("tail", 1, " beside it", 160., 0., 50.),
        ];

        let landing = land_selection(
            &participants,
            SelectionScopeId::ROOT,
            point(px(130.), px(10.)),
        );
        match landing {
            Landing::After(participant) => assert_eq!(participant.key.as_str(), "lead"),
            other => panic!("a press in a gap landed elsewhere: {}", other.name()),
        }
    }

    #[test]
    fn a_press_in_the_leading_margin_belongs_to_the_start_of_the_row() {
        let participants = vec![
            placed("lead", 0, "a paragraph with ", 40., 0., 100.),
            placed("tail", 1, " beside it", 140., 0., 50.),
        ];

        let landing = land_selection(
            &participants,
            SelectionScopeId::ROOT,
            point(px(10.), px(10.)),
        );
        match landing {
            Landing::Before(participant) => assert_eq!(participant.key.as_str(), "lead"),
            other => panic!("a press in the margin landed elsewhere: {}", other.name()),
        }
    }

    #[test]
    fn exactly_one_participant_owns_a_drag() {
        let mut selection = DocumentSelectionState::default();
        let a = SelectionContentKey::new("a".to_owned());
        let b = SelectionContentKey::new("b".to_owned());
        selection.begin(
            SelectionScopeId::ROOT,
            endpoint("a", 0, 2),
            2..2,
            SelectionUnit::Caret,
        );

        assert!(selection.owns_drag(&a));
        assert!(
            !selection.owns_drag(&b),
            "a participant the drag merely reached does not also answer for it"
        );

        selection.extend_to(SelectionScopeId::ROOT, endpoint("b", 1, 3));
        assert!(
            selection.owns_drag(&a),
            "ownership follows the press, not the pointer"
        );

        selection.end_drag();
        assert!(!selection.owns_drag(&a));
    }

    #[test]
    fn a_word_drag_snaps_to_the_word_it_reached_in_another_participant() {
        let mut selection = DocumentSelectionState::default();
        selection.begin(
            SelectionScopeId::ROOT,
            endpoint("a", 0, 0),
            0..5,
            SelectionUnit::Word,
        );

        // Landing mid-word in the next participant selects that whole word,
        // not the bytes the pointer happened to stop on.
        selection.extend_to_snapped(SelectionScopeId::ROOT, endpoint("b", 1, 8), Some(6..11));

        let b = SelectionContentKey::new("b".to_owned());
        assert_eq!(
            selection.range_for(&b, SelectionScopeId::ROOT, 1, 20),
            Some(0..11)
        );
    }

    #[test]
    fn a_row_drag_backwards_snaps_to_the_start_of_the_row_it_reached() {
        let mut selection = DocumentSelectionState::default();
        selection.begin(
            SelectionScopeId::ROOT,
            endpoint("b", 1, 0),
            4..9,
            SelectionUnit::Row,
        );

        selection.extend_to_snapped(SelectionScopeId::ROOT, endpoint("a", 0, 7), Some(3..12));

        let a = SelectionContentKey::new("a".to_owned());
        assert_eq!(
            selection.range_for(&a, SelectionScopeId::ROOT, 0, 20),
            Some(3..20),
            "a backwards row drag takes the reached row from its start"
        );
    }

    #[test]
    fn a_caret_drag_is_never_snapped() {
        let mut selection = DocumentSelectionState::default();
        selection.begin(
            SelectionScopeId::ROOT,
            endpoint("a", 0, 2),
            2..2,
            SelectionUnit::Caret,
        );
        let participants = vec![participant("a", 0, "one two three", 0.)];
        let a = SelectionContentKey::new("a".to_owned());

        assert_eq!(
            selection_unit_range(&participants, &a, 5, selection.drag_kind()),
            None
        );
    }

    #[test]
    fn shift_extending_without_an_anchor_places_a_caret() {
        let mut selection = DocumentSelectionState::default();
        selection.begin_shift_extend(SelectionScopeId::ROOT, endpoint("a", 0, 4));

        let a = SelectionContentKey::new("a".to_owned());
        assert_eq!(
            selection.range_for(&a, SelectionScopeId::ROOT, 0, 10),
            Some(4..4),
            "a Shift press with nothing selected is an ordinary caret press"
        );
        assert!(selection.owns_drag(&a));
    }

    #[test]
    fn shift_extending_keeps_the_existing_anchor() {
        let mut selection = DocumentSelectionState::default();
        selection.begin(
            SelectionScopeId::ROOT,
            endpoint("a", 0, 2),
            2..2,
            SelectionUnit::Caret,
        );
        selection.end_drag();

        selection.begin_shift_extend(SelectionScopeId::ROOT, endpoint("b", 1, 3));

        let a = SelectionContentKey::new("a".to_owned());
        let b = SelectionContentKey::new("b".to_owned());
        assert_eq!(
            selection.range_for(&a, SelectionScopeId::ROOT, 0, 6),
            Some(2..6)
        );
        assert_eq!(
            selection.range_for(&b, SelectionScopeId::ROOT, 1, 6),
            Some(0..3)
        );
        assert!(
            selection.owns_drag(&b),
            "the Shift press owns the drag it opened"
        );
    }

    #[test]
    fn a_word_unit_is_read_from_the_participant_that_was_reached() {
        let participants = vec![
            participant("a", 0, "alpha beta", 0.),
            participant("b", 1, "gamma delta", 20.),
        ];
        let b = SelectionContentKey::new("b".to_owned());

        assert_eq!(
            selection_unit_range(&participants, &b, 8, SelectionUnit::Word),
            Some(6..11),
            "the word comes from the reached participant's own text"
        );
    }

    #[test]
    fn a_unit_for_an_unmounted_participant_is_unknown() {
        let participants = vec![participant("a", 0, "alpha beta", 0.)];
        let gone = SelectionContentKey::new("gone".to_owned());

        assert_eq!(
            selection_unit_range(&participants, &gone, 3, SelectionUnit::Word),
            None,
            "GPUI does not invent boundaries for text it never rendered"
        );
    }

    #[test]
    fn an_endpoint_inside_a_participant_uses_its_resolver() {
        let mut entry = participant("a", 0, "hello world", 0.);
        entry.entry.resolve = Some(Rc::new(|position| position.x.0 as usize));
        let participants = vec![entry];

        let resolved = selection_endpoint_at(
            &participants,
            SelectionScopeId::ROOT,
            point(px(4.), px(10.)),
        );
        assert_eq!(resolved, Some(endpoint("a", 0, 4)));
    }

    #[test]
    fn an_endpoint_without_a_resolver_lands_at_the_participant_end() {
        let participants = vec![participant("a", 0, "hello", 0.)];

        let resolved = selection_endpoint_at(
            &participants,
            SelectionScopeId::ROOT,
            point(px(4.), px(10.)),
        );
        assert_eq!(
            resolved,
            Some(endpoint("a", 0, 5)),
            "an element that reported no mapping is selected to its end rather than guessed at"
        );
    }

    #[test]
    fn a_span_covers_the_middle_participant_in_full() {
        let selection = span(endpoint("a", 0, 3), endpoint("c", 2, 2));

        let key = SelectionContentKey::new("b".to_owned());
        assert_eq!(
            selection.range_for(&key, SelectionScopeId::ROOT, 1, 7),
            Some(0..7),
            "a participant between the two ends is covered without being consulted"
        );
    }

    #[test]
    fn a_participant_outside_the_span_is_not_covered() {
        let selection = span(endpoint("a", 1, 0), endpoint("b", 2, 4));

        let key = SelectionContentKey::new("z".to_owned());
        assert_eq!(
            selection.range_for(&key, SelectionScopeId::ROOT, 9, 5),
            None
        );
    }

    #[test]
    fn a_reverse_drag_resolves_the_same_span() {
        let forward = span(endpoint("a", 0, 2), endpoint("b", 1, 3));

        let reverse = span(endpoint("b", 1, 3), endpoint("a", 0, 2));

        let a = SelectionContentKey::new("a".to_owned());
        let b = SelectionContentKey::new("b".to_owned());
        assert_eq!(
            forward.range_for(&a, SelectionScopeId::ROOT, 0, 6),
            reverse.range_for(&a, SelectionScopeId::ROOT, 0, 6)
        );
        assert_eq!(
            forward.range_for(&b, SelectionScopeId::ROOT, 1, 6),
            reverse.range_for(&b, SelectionScopeId::ROOT, 1, 6)
        );
    }

    #[test]
    fn another_scope_is_never_covered() {
        let selection = span(endpoint("a", 0, 0), endpoint("c", 5, 3));

        let key = SelectionContentKey::new("modal".to_owned());
        assert_eq!(
            selection.range_for(&key, SelectionScopeId::new("dialog"), 1, 10),
            None,
            "a modal's text is not part of the document behind it"
        );
    }

    #[test]
    fn a_copy_joins_participants_in_reading_order() {
        let participants = vec![
            participant("b", 1, "second", 20.),
            participant("a", 0, "first", 0.),
            participant("c", 2, "third", 40.),
        ];
        let selection = span(endpoint("a", 0, 0), endpoint("c", 2, 5));

        let copy = selection.copy(&participants).expect("a span was selected");
        assert_eq!(copy.text, "first\nsecond\nthird");
        assert_eq!(copy.participants, 3);
        assert!(copy.complete);
    }

    #[test]
    fn a_copy_reports_an_unmounted_endpoint() {
        let participants = vec![participant("a", 0, "first", 0.)];
        let selection = span(endpoint("a", 0, 0), endpoint("gone", 9, 4));

        let copy = selection.copy(&participants).expect("a span was selected");
        assert_eq!(copy.text, "first");
        assert!(
            !copy.complete,
            "a copy whose far end is unmounted must say so rather than claim the part it saw"
        );
    }

    #[test]
    fn a_copy_across_a_virtualized_run_is_incomplete() {
        let mut rows = vec![
            participant("row.1", 1, "one", 0.),
            participant("row.9", 9, "nine", 20.),
        ];
        for row in &mut rows {
            row.entry.coverage = SelectionCoverage::Virtualized;
        }
        let selection = span(endpoint("row.1", 1, 0), endpoint("row.9", 9, 4));

        let copy = selection.copy(&rows).expect("a span was selected");
        assert_eq!(copy.text, "one\nnine");
        assert!(
            !copy.complete,
            "rows between two mounted rows were never rendered, so the copy cannot vouch for them"
        );
    }

    #[test]
    fn a_single_virtualized_participant_still_copies_completely() {
        let mut row = participant("row.1", 1, "one", 0.);
        row.entry.coverage = SelectionCoverage::Virtualized;
        let selection = span(endpoint("row.1", 1, 0), endpoint("row.1", 1, 3));

        let copy = selection.copy(&[row]).expect("a span was selected");
        assert_eq!(copy.text, "one");
        assert!(copy.complete, "one row can vouch for itself");
    }

    #[test]
    fn select_all_reaches_both_ends_of_its_scope() {
        let participants = vec![
            participant("a", 0, "first", 0.),
            participant("b", 1, "second", 20.),
        ];
        let mut selection = DocumentSelectionState::default();
        selection.select_all(SelectionScopeId::ROOT, &participants);

        let copy = selection.copy(&participants).expect("everything selected");
        assert_eq!(copy.text, "first\nsecond");
        assert!(copy.complete);
    }

    #[test]
    fn a_word_drag_keeps_the_whole_word_when_it_reverses() {
        let mut selection = DocumentSelectionState::default();
        selection.begin(
            SelectionScopeId::ROOT,
            endpoint("a", 1, 4),
            4..9,
            SelectionUnit::Word,
        );
        selection.extend_to(SelectionScopeId::ROOT, endpoint("a", 1, 1));

        let key = SelectionContentKey::new("a".to_owned());
        assert_eq!(
            selection.range_for(&key, SelectionScopeId::ROOT, 1, 20),
            Some(1..9),
            "dragging back past a word keeps that word's far edge anchored"
        );
    }

    #[test]
    fn a_drag_into_another_scope_is_refused() {
        let mut selection = DocumentSelectionState::default();
        selection.begin(
            SelectionScopeId::ROOT,
            endpoint("a", 0, 0),
            0..0,
            SelectionUnit::Caret,
        );
        selection.extend_to(SelectionScopeId::new("dialog"), endpoint("modal", 0, 5));

        assert!(
            selection.is_empty(),
            "a drag cannot leave the scope it started in"
        );
    }

    #[test]
    fn landing_between_participants_extends_to_the_edge_it_passed() {
        let participants = vec![
            participant("a", 0, "first", 0.),
            participant("b", 1, "second", 40.),
        ];
        match land_selection(
            &participants,
            SelectionScopeId::ROOT,
            point(px(5.), px(30.)),
        ) {
            Landing::After(participant) => assert_eq!(participant.key.as_str(), "a"),
            _ => panic!("a position in the gap belongs to the participant above it"),
        }
        match land_selection(
            &participants,
            SelectionScopeId::ROOT,
            point(px(5.), px(-10.)),
        ) {
            Landing::Before(participant) => assert_eq!(participant.key.as_str(), "a"),
            _ => panic!("a position above everything belongs to the first participant"),
        }
    }

    #[test]
    fn autoscroll_reports_only_the_overflow() {
        let participants = vec![participant("a", 0, "first", 0.)];
        assert_eq!(
            selection_autoscroll_for(
                &participants,
                SelectionScopeId::ROOT,
                point(px(0.), px(10.))
            )
            .y,
            px(0.),
            "a pointer inside the content asks for no scrolling"
        );
        assert_eq!(
            selection_autoscroll_for(
                &participants,
                SelectionScopeId::ROOT,
                point(px(0.), px(35.))
            )
            .y,
            px(15.)
        );
        assert_eq!(
            selection_autoscroll_for(
                &participants,
                SelectionScopeId::ROOT,
                point(px(0.), px(-5.))
            )
            .y,
            px(-5.)
        );
    }

    #[test]
    fn a_sensitive_participant_never_registers() {
        let mut map = FxHashMap::default();
        let secret = SelectionParticipant::new(
            SelectionContentKey::new("token".to_owned()),
            0,
            Bounds::new(point(px(0.), px(0.)), size(px(10.), px(10.))),
        )
        .text("sk-live-000")
        .sensitive(true);
        document_selection_register(&mut map, &secret);
        assert!(
            map.is_empty(),
            "a credential must not become reachable through an aggregate copy"
        );
    }

    #[test]
    fn the_same_business_key_can_exist_in_two_scopes() {
        let mut map = FxHashMap::default();
        let bounds = Bounds::new(point(px(0.), px(0.)), size(px(10.), px(10.)));
        let page = SelectionParticipant::new("body", 0, bounds).text("page");
        let dialog = SelectionParticipant::new("body", 0, bounds)
            .scope(SelectionScopeId::new("dialog"))
            .text("dialog");

        document_selection_register(&mut map, &page);
        document_selection_register(&mut map, &dialog);

        assert_eq!(
            document_selection_registered(&map).len(),
            2,
            "scope is part of registration identity, so an overlay cannot replace the page participant"
        );
    }

    #[test]
    fn autoscroll_is_zero_unless_a_drag_is_running() {
        let mut selection = DocumentSelectionState::default();
        selection.set_autoscroll(Point::new(Pixels::ZERO, px(12.)));
        assert_eq!(selection.autoscroll_delta().y, px(0.));
        selection.dragging = true;
        assert_eq!(selection.autoscroll_delta().y, px(12.));
    }
}
