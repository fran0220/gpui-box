//! The words this library puts on screen, and the host's right to replace
//! them.
//!
//! Components never hold English. They hold a [`StringKey`], and ask the
//! application context for the text behind it at render time, exactly the way
//! they ask for a colour:
//!
//! ```no_run
//! # use gpui_kit::strings::{ActiveStrings, StringKey};
//! # fn example(cx: &gpui::App) -> gpui::SharedString {
//! cx.strings().text(StringKey::Copy)
//! # }
//! ```
//!
//! Every key carries an English default compiled into the binary, so a host
//! that supplies nothing still gets a working, readable interface. A host that
//! supplies some keys gets its own words for those and English for the rest;
//! there is no state in which a label renders empty.
//!
//! Strings with a value in them are templates over positional placeholders —
//! `{0}`, `{1}` — rather than Rust format strings, because a translation is
//! allowed to put the value somewhere else in the sentence:
//!
//! ```no_run
//! # use gpui_kit::strings::{ActiveStrings, StringKey};
//! # fn example(cx: &gpui::App, query: &str) -> gpui::SharedString {
//! cx.strings().format(StringKey::PaletteNoMatch, &[query])
//! # }
//! ```
//!
//! # What is not here
//!
//! Numbers, dates, and quantities are not translated. A count is still
//! rendered with Rust's own digits and a plural is still chosen by an
//! `if count == 1` at the call site, which is correct for English and for
//! nothing else. `docs/coverage.md` records that as a named gap.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use gpui::{App, BorrowAppContext, Global, SharedString};

/// Declares every key once: the variant, the stable name a host and a test
/// use to address it, and the English a host gets for free.
macro_rules! string_keys {
    ($( $variant:ident => $name:literal, $default:literal ; )*) => {
        /// Every piece of text this library can put on a screen.
        ///
        /// The set is closed and exhaustive: a component that needs a new word
        /// adds a variant here, which is what makes a missing translation a
        /// compile-time question rather than a runtime blank.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[non_exhaustive]
        pub enum StringKey {
            $( #[doc = $default] $variant, )*
        }

        impl StringKey {
            /// Every key, in declaration order.
            pub const ALL: &'static [StringKey] = &[ $( StringKey::$variant, )* ];

            /// The stable name a host configuration or a test uses.
            ///
            /// It is not derived from the variant name at runtime, so renaming
            /// a variant cannot silently rename a host's configuration key.
            pub const fn name(self) -> &'static str {
                match self {
                    $( StringKey::$variant => $name, )*
                }
            }

            /// The English compiled into the binary.
            pub const fn english(self) -> &'static str {
                match self {
                    $( StringKey::$variant => $default, )*
                }
            }

            /// Looks a key up by its stable name.
            pub fn from_name(name: &str) -> Option<Self> {
                StringKey::ALL.iter().copied().find(|key| key.name() == name)
            }
        }
    };
}

string_keys! {
    // Shared vocabulary. One word used by more than one component is one key,
    // so a host that renames it renames it everywhere it appears.
    Copy => "common.copy", "Copy";
    Dismiss => "common.dismiss", "Dismiss";
    TryAgain => "common.try-again", "Try again";
    Loading => "common.loading", "Loading";
    MoreActions => "common.more-actions", "More actions";
    Expand => "common.expand", "Expand";
    Collapse => "common.collapse", "Collapse";

    // Calendar.
    CalendarNoMonth => "calendar.no-month", "No month to show";
    CalendarPreviousMonth => "calendar.previous-month", "Previous month";
    CalendarNextMonth => "calendar.next-month", "Next month";
    CalendarUnknownMonth => "calendar.unknown-month", "This calendar does not know which month to show";
    CalendarUnknownMonthDetail => "calendar.unknown-month-detail", "Nothing is selected, the host has not said what day it is, and no month was given.";

    // Date field.
    DateInputOpen => "date-input.open", "Open the calendar";
    DateInputPlaceholder => "date-input.placeholder", "Date";

    // Range picker.
    RangeUnset => "range.unset", "No range chosen yet.";
    RangeIncomplete => "range.incomplete", "{0} to an end that has not been chosen yet.";
    RangeComplete => "range.complete", "{0} to {1}.";
    RangeInverted => "range.inverted", "The end, {0}, comes before the start, {1}.";
    RangeUncheckable => "range.uncheckable", "The host cannot list the days in this range, so none of them were checked.";
    RangeBlockedDay => "range.blocked-day", "{0}: {1}";

    // Time field.
    TimeHour => "time.hour", "Hour";
    TimeMinute => "time.minute", "Minute";
    TimeSecond => "time.second", "Second";
    TimeMeridiem => "time.meridiem", "Half of the day";

    // Dock, split, scroll, toolbar.
    DockCollapseRegion => "dock.collapse-region", "Collapse region";
    SplitResizeHandle => "split.resize-handle", "Resize panes";
    StatusStale => "status.stale", "stale";
    ScrollbarVertical => "scrollbar.vertical", "Vertical";
    ScrollbarHorizontal => "scrollbar.horizontal", "Horizontal";

    // Breadcrumb.
    BreadcrumbHiddenOne => "breadcrumb.hidden-one", "1 hidden level";
    BreadcrumbHiddenMany => "breadcrumb.hidden-many", "{0} hidden levels";

    // Pagination.
    PaginationFirst => "pagination.first", "First page";
    PaginationPrevious => "pagination.previous", "Previous page";
    PaginationNext => "pagination.next", "Next page";
    PaginationLast => "pagination.last", "Last page";
    PaginationMorePages => "pagination.more-pages", "{0} more pages";
    PaginationPageOfTotal => "pagination.page-of-total", "Page {0} of {1}";
    PaginationPage => "pagination.page", "Page {0}";

    // Wizard.
    WizardBack => "wizard.back", "Back";
    WizardNext => "wizard.next", "Next";
    WizardFinish => "wizard.finish", "Finish";
    WizardReturnsTo => "wizard.returns-to", "Returns to";

    // Image viewer.
    ImageViewerNotSupplied => "image-viewer.not-supplied", "Not supplied — {0}";
    ImageViewerPrevious => "image-viewer.previous", "Previous image";
    ImageViewerNext => "image-viewer.next", "Next image";
    ImageViewerContain => "image-viewer.contain", "Contain";
    ImageViewerCover => "image-viewer.cover", "Cover";
    ImageViewerSizeUnknown => "image-viewer.size-unknown", "Size unknown";
    ImageViewerEmpty => "image-viewer.empty", "No images";

    // Markdown.
    MarkdownImageAlt => "markdown.image-alt", "Image";
    MarkdownImageNotFetched => "markdown.image-not-fetched", "Not fetched — {0}";
    MarkdownPlainText => "markdown.plain-text", "plain text";
    MarkdownTask => "markdown.task", "Task";
    MarkdownUnrenderedHtml => "markdown.unrendered-html", "unrendered html";
    MarkdownShowMoreOne => "markdown.show-more-one", "Show 1 more line";
    MarkdownShowMoreMany => "markdown.show-more-many", "Show {0} more lines";

    // Conversation.
    MessageSending => "message.sending", "Sending";
    MessageSent => "message.sent", "Sent";
    MessageDelivered => "message.delivered", "Delivered";
    MessageRead => "message.read", "Read";
    MessageStreaming => "message.streaming", "Streaming";
    MessageMoreOne => "message.more-one", "1 more message";
    MessageMoreMany => "message.more-many", "{0} more messages";
    MessageNewOne => "message.new-one", "1 new message";
    MessageNewMany => "message.new-many", "{0} new messages";
    MessageShowMoreOne => "message.show-more-one", "1 more line";
    MessageShowMoreMany => "message.show-more-many", "{0} more lines";
    TimeUnknown => "time.unknown", "Time unknown";

    // Transport bar.
    TransportBuffered => "transport.buffered", "Buffered";
    TransportTimeUnknown => "transport.time-unknown", "Time unknown";
    TransportDurationUnknown => "transport.duration-unknown", "Duration unknown";
    TransportPosition => "transport.position", "Playback position";
    TransportPlay => "transport.play", "Play";
    TransportPause => "transport.pause", "Pause";
    TransportPlaying => "transport.playing", "Playing";
    TransportPaused => "transport.paused", "Paused";
    TransportBuffering => "transport.buffering", "Waiting for data";
    TransportMute => "transport.mute", "Mute";
    TransportUnmute => "transport.unmute", "Unmute";
    TransportVolume => "transport.volume", "Volume";
    TransportPreviousTrack => "transport.previous-track", "Previous track";
    TransportNextTrack => "transport.next-track", "Next track";

    // Combobox and select.
    SelectPlaceholder => "select.placeholder", "Select";
    ComboboxNoMatch => "combobox.no-match", "Nothing here answers “{0}”";
    ComboboxCreateHint => "combobox.create-hint", "Press enter to add it as a new value.";
    ComboboxClosedHint => "combobox.closed-hint", "This field only accepts one of the options offered.";

    // Drop zone.
    DropzoneRefusal => "dropzone.refusal", "This zone does not take that.";

    // Filter bar.
    FilterBarLabel => "filter-bar.label", "Filters";
    FilterBarAdd => "filter-bar.add", "Add filter";
    FilterBarClear => "filter-bar.clear", "Clear all";
    FilterBarCounting => "filter-bar.counting", "Counting…";
    FilterBarResultsNoun => "filter-bar.results-noun", "results";

    // Inline edit and keybinding recorder.
    InlineEditPlaceholder => "inline-edit.placeholder", "Empty";
    KeybindingUnbound => "keybinding.unbound", "Not bound";
    KeybindingPrompt => "keybinding.prompt", "Press a shortcut";

    // Number field.
    NumberDecrease => "number.decrease", "Decrease";
    NumberIncrease => "number.increase", "Increase";
    NumberNotANumber => "number.not-a-number", "This is not a number.";
    NumberBelowMinimum => "number.below-minimum", "The smallest accepted value is {0}.";
    NumberAboveMaximum => "number.above-maximum", "The largest accepted value is {0}.";

    // Settings row.
    SettingsManagedBy => "settings.managed-by", "Managed by {0}";
    SettingsInapplicable => "settings.inapplicable", "Not available here";

    // Tag field and tag.
    TagInputPlaceholder => "tag-input.placeholder", "Add";
    TagInputDuplicate => "tag-input.duplicate", "“{0}” is already here";
    TagInputFull => "tag-input.full", "This field holds at most {0}; “{1}” was not added";
    TagInputUsed => "tag-input.used", "{0} of {1} used";
    TagRemove => "tag.remove", "Remove {0}";

    // Description list.
    DescriptionUnknown => "description.unknown", "Unknown";
    DescriptionNotApplicable => "description.not-applicable", "Not applicable";
    DescriptionCopy => "description.copy", "Copy {0}";
    DescriptionCharacters => "description.characters", "{0} characters";

    // How a position in a run of things is worded. It is one key, because a
    // reader who learns it on a progress bar should read the same shape on an
    // image caption.
    CountOfTotal => "common.count-of-total", "{0} of {1}";

    // Command palette.
    PalettePlaceholder => "palette.placeholder", "Type a command";
    PaletteNoMatch => "palette.no-match", "No command matches “{0}”";
    PaletteEmptyDetail => "palette.empty-detail", "Every command this application was given is listed here.";

    // Keystroke names. The glyphs a Mac shows are not words and are not here;
    // these are the spelled-out forms every other platform reads.
    KbdSuper => "kbd.super", "Win";
    KbdControl => "kbd.control", "Ctrl";
    KbdAlt => "kbd.alt", "Alt";
    KbdShift => "kbd.shift", "Shift";

    // Data grid.
    GridSelectedNoun => "grid.selected-noun", "selected";
    GridSelectAllLoaded => "grid.select-all-loaded", "Select all loaded rows";
    GridSelectAllTotal => "grid.select-all-total", "Select all {0}";
    GridClearSelection => "grid.clear-selection", "Clear selection";
    GridResizeColumn => "grid.resize-column", "Resize {0}";
    GridLoadingRows => "grid.loading-rows", "Loading rows";
    GridLoadFailed => "grid.load-failed", "Could not load rows";
    GridEmpty => "grid.empty", "No rows";

    // Drag and drop.
    DragFileOne => "drag.file-one", "1 file";
    DragFileMany => "drag.file-many", "{0} files";

    // Copy button. The confirmation and the refusal are separate keys because
    // they are separate claims: one says the clipboard took the text and the
    // other says it did not, and a host wording them must not be able to
    // collapse the two into the same sentence.
    CopyDone => "copy.done", "Copied";
    CopyFailed => "copy.failed", "Not copied";
    CopyFailedDetail => "copy.failed-detail", "The clipboard did not take it.";

    // Approval. The scope of an "always" is part of the wording on the
    // control, so there is no key here for an unscoped one to be worded with.
    ApprovalDecline => "approval.decline", "Decline";
    ApprovalApproveOnce => "approval.approve-once", "Approve once";
    ApprovalAlwaysSession => "approval.always-session", "Always for this session";
    ApprovalAlwaysTool => "approval.always-tool", "Always for {0}";
    ApprovalAlwaysPath => "approval.always-path", "Always in {0}";
    ApprovalAlwaysHost => "approval.always-host", "Always on {0}";
    ApprovalPending => "approval.pending", "Waiting for your answer";
    ApprovalDeclined => "approval.declined", "Declined";
    ApprovalApproved => "approval.approved", "Approved: {0}";
    ApprovalOnceScope => "approval.once-scope", "this time only";
    ApprovalExpired => "approval.expired", "This request expired before it was answered";
    ApprovalSuperseded => "approval.superseded", "Replaced by {0}";

    // Permission matrix.
    PermissionAllowed => "permission.allowed", "Allowed";
    PermissionDenied => "permission.denied", "Denied";
    PermissionAsk => "permission.ask", "Ask every time";
    PermissionNotApplicable => "permission.not-applicable", "Does not apply";
    PermissionSubjectHeading => "permission.subject-heading", "Subject";
    PermissionInherited => "permission.inherited", "Inherited from {0}";
    PermissionSetHere => "permission.set-here", "Set here";
    PermissionCellName => "permission.cell-name", "{0}: {1}";

    // Cost and context. An estimate carries its label inside the value, so a
    // reading cannot be worded without saying which of the two it is.
    CostMeasured => "cost.measured", "{0}";
    CostEstimated => "cost.estimated", "{0} (estimated)";
    CostEstimateMark => "cost.estimate-mark", "Estimate";
    CostUnavailable => "cost.unavailable", "Unavailable";
    CostLastVerified => "cost.last-verified", "Last verified {0}";
    ContextUnknownLimit => "context.unknown-limit", "Limit unknown";
    // Structured value view. `null`, `true`, `{}` and `[]` are JSON syntax
    // rather than words, so they are not here: translating them would produce
    // a document nobody could paste back.
    JsonWithheld => "json.withheld", "withheld";
    JsonRootValue => "json.root-value", "Value";
    JsonShapeEntries => "json.shape-entries", "{0} entries";
    JsonShapeItems => "json.shape-items", "{0} items";
    JsonShapeValue => "json.shape-value", "a value";

    // Schema-generated form.
    SchemaUnrenderable => "schema.unrenderable", "This field cannot be shown here, so it has to be filled in some other way.";
    SchemaUnrenderableRequired => "schema.unrenderable-required", "This field is required and cannot be shown here, so this form cannot complete the call.";
    SchemaNoChoices => "schema.no-choices", "No choices were offered, so there is nothing to pick.";
    SchemaRequiredMissing => "schema.required-missing", "This field is required.";
    SchemaUnrenderableOne => "schema.unrenderable-one", "1 field cannot be shown here.";
    SchemaUnrenderableMany => "schema.unrenderable-many", "{0} fields cannot be shown here.";

    // Connections, and what each one offers.
    ServerConnected => "server.connected", "Connected";
    ServerConnecting => "server.connecting", "Connecting";
    ServerDisconnected => "server.disconnected", "Disconnected";
    ServerFailed => "server.failed", "Failed";
    ServerDisabled => "server.disabled", "Turned off";
    ServerTools => "server.tools", "Tools";
    ServerSkills => "server.skills", "Skills";
    ServerResources => "server.resources", "Resources";
    ServerOfferingsUnasked => "server.offerings-unasked", "Nothing has been asked for yet";
    ServerOfferingsUnaskedDetail => "server.offerings-unasked-detail", "This connection has not been asked what it offers.";
    ServerOfferingsAsking => "server.offerings-asking", "Asking what this connection offers";
    ServerOfferingsNone => "server.offerings-none", "This connection offers nothing";
    ServerOfferingsNoneDetail => "server.offerings-none-detail", "It answered, and the answer was empty.";
    ServerOfferingsUnavailable => "server.offerings-unavailable", "What this connection offers is unknown";
    ServerEmpty => "server.empty", "No connections";
    ServerEmptyDetail => "server.empty-detail", "Nothing has been connected yet.";

    // A run: one tool call, the steps it belongs to, and the reasoning beside
    // them. The state words are shared between the card and the list, because
    // a call that is running and a step that is running are one word to a
    // reader.
    AgentArguments => "agent.arguments", "Arguments";
    AgentResult => "agent.result", "Result";
    AgentPendingApproval => "agent.pending-approval", "Waiting for approval";
    AgentRunning => "agent.running", "Running";
    AgentSucceeded => "agent.succeeded", "Succeeded";
    AgentFailed => "agent.failed", "Failed";
    AgentDeclined => "agent.declined", "Declined";
    AgentPending => "agent.pending", "Pending";
    AgentDone => "agent.done", "Done";
    AgentSkipped => "agent.skipped", "Skipped";
    AgentNoOutput => "agent.no-output", "This tool returned nothing";
    AgentElapsedUnknown => "agent.elapsed-unknown", "Elapsed time unknown";
    AgentTruncated => "agent.truncated", "{0} of {1} lines shown";
    AgentLinesOne => "agent.lines-one", "1 line";
    AgentLinesMany => "agent.lines-many", "{0} lines";
    AgentStepsDoneOne => "agent.steps-done-one", "1 step done";
    AgentStepsDoneMany => "agent.steps-done-many", "{0} steps done";
    AgentReasoning => "agent.reasoning", "Reasoning";
    AgentReasoningWithheld => "agent.reasoning-withheld", "Withheld";
    AgentReasoningAbsent => "agent.reasoning-absent", "No reasoning was returned";

    // Document tabs. A clean tab carries no wording at all, which is why
    // there is no key here for one: silence is the whole message.
    TabDirty => "tab.dirty", "Unsaved changes";
    TabSaving => "tab.saving", "Saving";
    TabSaveFailed => "tab.save-failed", "Could not save";
    TabClose => "tab.close", "Close {0}";
    TabMoreTabs => "tab.more-tabs", "More tabs";

    // Search, and find and replace.
    SearchPlaceholder => "search.placeholder", "Find";
    SearchNoHits => "search.no-hits", "No results";
    SearchCounting => "search.counting", "Counting…";
    SearchNotSearched => "search.not-searched", "Nothing searched yet";
    SearchTooMany => "search.too-many", "More than {0}";
    SearchHitOne => "search.hit-one", "1 result";
    SearchHitMany => "search.hit-many", "{0} results";
    SearchNext => "search.next", "Next result";
    SearchPrevious => "search.previous", "Previous result";
    SearchCaseSensitive => "search.case-sensitive", "Match case";
    SearchWholeWord => "search.whole-word", "Whole word";
    ReplacePlaceholder => "replace.placeholder", "Replace with";
    ReplaceOne => "replace.one", "Replace";
    ReplaceAllCounted => "replace.all-counted", "Replace all {0}";
    ReplaceAllUncounted => "replace.all-uncounted", "Replace all";
    ReplaceAllUncountable => "replace.all-uncountable", "Nobody has counted the results yet, so this cannot say how many it would change.";

    // Notification centre.
    NotificationsTitle => "notifications.title", "Notifications";
    NotificationsEmpty => "notifications.empty", "Nothing to report";
    NotificationsEmptyDetail => "notifications.empty-detail", "Notifications that have come and gone are kept here.";
    NotificationsClearAll => "notifications.clear-all", "Clear all";
    NotificationsMarkAllRead => "notifications.mark-all-read", "Mark all as read";
    NotificationsUnread => "notifications.unread", "Unread";
    NotificationsUnreadCount => "notifications.unread-count", "{0} unread";
    NotificationsAtLeast => "notifications.at-least", "{0}+";

    // A panel whose contents the host could not produce.
    FailureTitle => "failure.title", "This panel could not be shown";
    FailureAttempts => "failure.attempts", "Tried {0} times";
    FailureRetrying => "failure.retrying", "Trying again";

    // Read-only code.
    CodeLineNumbers => "code.line-numbers", "Line numbers";
    CodeLineAdded => "code.line-added", "Added";
    CodeLineRemoved => "code.line-removed", "Removed";
    CodeLineChanged => "code.line-changed", "Changed";
    CodeLineHighlighted => "code.line-highlighted", "Highlighted";
    CodeLineError => "code.line-error", "Error";
    CodeEmpty => "code.empty", "Nothing to show";

    // Uploads.
    UploadQueued => "upload.queued", "Queued";
    UploadUploading => "upload.uploading", "Uploading";
    UploadDone => "upload.done", "Uploaded";
    UploadFailed => "upload.failed", "Failed";
    UploadCancelled => "upload.cancelled", "Cancelled";
    UploadRefused => "upload.refused", "Not accepted";
    UploadCancel => "upload.cancel", "Cancel {0}";
    UploadRemove => "upload.remove", "Remove {0}";
    UploadOverall => "upload.overall", "Uploading";
    UploadEmpty => "upload.empty", "No files yet";

    // The web view shell. The four things that are not a page each say a
    // different thing, so each of them is its own key rather than one
    // "cannot show" a host would have to disambiguate by guessing.
    BrowserBack => "browser.back", "Back";
    BrowserForward => "browser.forward", "Forward";
    BrowserReload => "browser.reload", "Reload";
    BrowserNoAddress => "browser.no-address", "No address";
    BrowserNoEngine => "browser.no-engine", "No web engine";
    BrowserNoEngineDetail => "browser.no-engine-detail", "This build cannot display web content.";
    BrowserRefused => "browser.refused", "Address not opened";
    BrowserFailed => "browser.failed", "Could not load";
    BrowserNoViewport => "browser.no-viewport", "No page surface";
    BrowserNoViewportDetail => "browser.no-viewport-detail",
        "The host reported a ready page but supplied no viewport.";
}

/// The catalogue a host installs, and the one components read.
///
/// It holds only the entries a host replaced. An absent entry is not a gap to
/// be filled at runtime; it means the English default stands, which is why a
/// partial catalogue is a legitimate thing to install rather than a mistake.
#[derive(Debug, Clone, Default)]
pub struct Strings {
    overrides: BTreeMap<StringKey, SharedString>,
}

impl Global for Strings {}

impl Strings {
    /// An empty catalogue: every key answers with its English.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces one entry.
    pub fn set(&mut self, key: StringKey, text: impl Into<SharedString>) -> &mut Self {
        self.overrides.insert(key, text.into());
        self
    }

    /// Restores the English for one entry.
    pub fn clear(&mut self, key: StringKey) -> &mut Self {
        self.overrides.remove(&key);
        self
    }

    /// Restores the English for every entry.
    pub fn clear_all(&mut self) -> &mut Self {
        self.overrides.clear();
        self
    }

    /// Replaces many entries, leaving the rest alone.
    pub fn extend(
        &mut self,
        entries: impl IntoIterator<Item = (StringKey, SharedString)>,
    ) -> &mut Self {
        self.overrides.extend(entries);
        self
    }

    /// Whether this key was replaced. A component never asks; a test does.
    pub fn is_overridden(&self, key: StringKey) -> bool {
        self.overrides.contains_key(&key)
    }

    /// The text behind a key, which is always something a reader can read.
    pub fn text(&self, key: StringKey) -> SharedString {
        match self.overrides.get(&key) {
            Some(text) => text.clone(),
            None => SharedString::new_static(key.english()),
        }
    }

    /// The text behind a key with `{0}`, `{1}`, … replaced in place.
    ///
    /// A placeholder with no argument is left standing rather than removed, so
    /// a wrong translation reads as an obvious mistake instead of a sentence
    /// that quietly lost a fact.
    pub fn format(&self, key: StringKey, args: &[&str]) -> SharedString {
        SharedString::from(interpolate(self.text(key).as_ref(), args))
    }
}

fn interpolate(template: &str, args: &[&str]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let (before, tail) = rest.split_at(open);
        out.push_str(before);
        let Some(close) = tail.find('}') else {
            out.push_str(tail);
            return out;
        };
        let slot = &tail[1..close];
        match slot.parse::<usize>().ok().and_then(|index| args.get(index)) {
            Some(value) => out.push_str(value),
            None => out.push_str(&tail[..=close]),
        }
        rest = &tail[close + 1..];
    }
    out.push_str(rest);
    out
}

/// Reads the installed catalogue from any context that dereferences to
/// [`App`], mirroring [`ActiveTheme`](gpui_kit_theme::ActiveTheme).
pub trait ActiveStrings {
    fn strings(&self) -> &Strings;
}

impl ActiveStrings for App {
    /// Falls back to the English catalogue when no host installed one, because
    /// a component that panicked or rendered blank for want of a global would
    /// be a worse library than one with English compiled in.
    fn strings(&self) -> &Strings {
        static ENGLISH: OnceLock<Strings> = OnceLock::new();
        self.try_global::<Strings>()
            .unwrap_or_else(|| ENGLISH.get_or_init(Strings::new))
    }
}

/// Installs the catalogue global. Idempotent, and never discards a catalogue a
/// host already installed.
pub fn install(cx: &mut App) {
    if !cx.has_global::<Strings>() {
        cx.set_global(Strings::new());
    }
}

/// Replaces entries and repaints every window, the way
/// [`activate_theme`](gpui_kit_theme::activate_theme) does.
pub fn set_strings(entries: impl IntoIterator<Item = (StringKey, SharedString)>, cx: &mut App) {
    install(cx);
    cx.update_global::<Strings, ()>(|strings, _| {
        strings.extend(entries);
    });
    cx.refresh_windows();
}

/// Restores the English for every entry and repaints every window.
pub fn reset_strings(cx: &mut App) {
    install(cx);
    cx.update_global::<Strings, ()>(|strings, _| {
        strings.clear_all();
    });
    cx.refresh_windows();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_has_a_unique_name_and_english() {
        let mut names: Vec<&str> = StringKey::ALL.iter().map(|key| key.name()).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before, "two keys share a name");
        for key in StringKey::ALL {
            assert!(!key.english().is_empty(), "{} has no English", key.name());
            assert_eq!(StringKey::from_name(key.name()), Some(*key));
        }
    }

    #[test]
    fn an_empty_catalogue_answers_in_english() {
        let strings = Strings::new();
        assert_eq!(strings.text(StringKey::Copy), "Copy");
        assert_eq!(strings.text(StringKey::TryAgain), "Try again");
    }

    #[test]
    fn an_override_replaces_only_what_it_names() {
        let mut strings = Strings::new();
        strings.set(StringKey::Copy, "Kopieren");
        assert_eq!(strings.text(StringKey::Copy), "Kopieren");
        assert_eq!(strings.text(StringKey::TryAgain), "Try again");
        strings.clear(StringKey::Copy);
        assert_eq!(strings.text(StringKey::Copy), "Copy");
    }

    #[test]
    fn a_template_takes_its_arguments_in_any_order() {
        let mut strings = Strings::new();
        assert_eq!(
            strings.format(StringKey::RangeComplete, &["Monday", "Friday"]),
            "Monday to Friday."
        );
        strings.set(StringKey::RangeComplete, "{1} back to {0}.");
        assert_eq!(
            strings.format(StringKey::RangeComplete, &["Monday", "Friday"]),
            "Friday back to Monday."
        );
    }

    #[test]
    fn a_placeholder_with_no_argument_stays_visible() {
        let strings = Strings::new();
        assert_eq!(
            strings.format(StringKey::RangeComplete, &["Monday"]),
            "Monday to {1}."
        );
    }

    #[test]
    fn every_placeholder_in_the_english_is_numbered_from_zero() {
        for key in StringKey::ALL {
            let english = key.english();
            let mut expected = 0usize;
            let mut rest = english;
            while let Some(open) = rest.find('{') {
                let tail = &rest[open..];
                let close = tail.find('}').unwrap_or_else(|| {
                    panic!("{} has an unclosed placeholder", key.name());
                });
                let slot = &tail[1..close];
                assert_eq!(
                    slot.parse::<usize>().ok(),
                    Some(expected),
                    "{} numbers its placeholders out of order",
                    key.name()
                );
                expected += 1;
                rest = &tail[close + 1..];
            }
        }
    }
}
