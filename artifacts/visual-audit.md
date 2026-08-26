# Visual audit — full catalog, 2026-08-24

Method: every one of the 136 scene baselines in
`snapshots/headless/macos/scenes/*-studio-dark.png` was reviewed at high
quality by six independent reviewers; the light variant was additionally read
wherever a colour or contrast question came up. Gradient sightings were
confirmed by pixel sampling against the three loader stops
(`#b6d3ef` / `#edb185` / `#f888a0`).

Severity: **BLOCKER** unusable or reads as broken · **MAJOR** clearly ugly or
wrong · **MINOR** polish. Check a box when the defect is fixed and the scene
baseline has been re-captured and inspected.

## Owner directives (the bar every fix is held to)

1. **No hard-coded colour, geometry, or motion.** Every repeated value goes
   through a token and stays overridable by the caller.
2. **Default is neutral grey.** Colour appears only where it carries meaning
   (semantic status, data). No decorative accent, no decorative gradients.
3. **One visual language.** Radius, control heights, spacing, glow budget,
   affordances agree across every component.
4. **Under-built components are finished to product grade**: structure,
   hierarchy, and states in one pass — except the agent family (thinking,
   tool-call, agent-document, step-list …), whose quiet row language is a
   recent deliberate redesign and is kept; only genuine defects are fixed
   there.

Counts to drive to zero at audit time: 32 hard-coded colour literals, 21
literal `rounded(px(..))` radii, 136 accent references (most decorative) in
`crates/gpui-kit/src` outside scenes.

---

## Root causes

| # | Root cause | Scenes hit | Worst |
|---|---|---|---|
| A | Three-stop loader gradient leaking into single-scalar fills (`display/signature.rs`) | 17 | BLOCKER |
| B | Skeleton/placeholder fills louder than real content (`colors.track` misused as placeholder) | 9 | BLOCKER |
| C | Content clipped mid-glyph at panel edges (no bottom padding / fade policy) | 9 | BLOCKER |
| D | Two distinct states rendered identically | 11 | BLOCKER |
| E | Bare text where an action affordance belongs | 8 | MAJOR |
| F | Indeterminate reads as complete or stuck at a still frame | 7 | BLOCKER |
| G | Track heavier than fill, inverting the reading | 4 | MAJOR |
| H | Glow/bloom overruns its container | 6 | MAJOR |
| I | Missing glyphs: tofu boxes, blank icon squares, wrong stand-in icons | 6 | BLOCKER |
| J | Under-built components, near-placeholder | 22 | BLOCKER |

- A: progress-bar, progress-circle, step-list, upload-list, motion-state,
  game-ui (x3), transport (x2), audio-player, video-player, ide-shell,
  loading, state-ladder, node-graph, artifact-preview, metric-card, terminal,
  avatar (identity ring)
- B: loading, state-ladder, node-graph, metric-card, artifact-preview,
  terminal, schema-form (dropzone), server-list ("turned off"), image-viewer /
  video-player (media placeholder)
- C: flow, outline, scroll-area, scroll-shadow, diff-view, chart, code-view,
  list, table
- D: feedback-rating, button, tabs, toggle, copy-button, agent-avatar,
  cost-meter, thinking, tree, sparkline, date-range
- E: progress-bar (Cancel), keymap-editor, tool-call, approval,
  notification-center, diagnostics-list, anchor-list, filter-bar
- F: progress-bar, progress-circle, step-list, browser-panel, thinking, tree,
  filter-bar
- G: progress-bar, progress-circle, step-list, motion-state
- H: failure-panel, outcome-panel, state-ladder, keybinding, agent-run-canvas,
  game-ui
- I: markdown, conversation, icon, upload-list, date-time, and the generic
  checklist glyph reused in prompt-builder / audio-waveform / agent-document
- J: micro, thinking (quiet-language fixes only), animated-number, sparkline,
  heatmap, avatar, badge, stage-progress, split-pane, responsive,
  canvas-tools, feedback-rating, kbd, divider, prompt-builder,
  audio-waveform, agent-run-issues, kanban, trace, drawer, wizard, accordion
  (json-view close behind)

Positive references (do not regress): `table` state pills, `list`/`tree`
selection rail, `command-palette`, `settings`, `dialog`, `empty-state`,
`server-list` status vocabulary, `card`, `terminal` ANSI rendering,
`agent-document`.

---

## Per-scene findings

### button
- [x] MAJOR Disabled and loading are the loudest chips on screen; light grey fill out-shouts enabled Secondary
- [x] MAJOR Disabled vs loading near-identical; only a faint spinner glyph differs
- [ ] MINOR Saving spinner nearly invisible on its pill
- [x] MINOR No section captions for variants / states / sizes rows
- [ ] MINOR Size ramp barely steps between Xs and Sm

### badge
- [x] MINOR Under-built: no dot/count/icon badge, no size ramp, no removable or outline variant, no captions

### card
- [ ] MINOR Footer actions sit tight against bottom edge; vertical padding asymmetric

### status
- [x] MAJOR Banner strips run full frame width while everything else stops at ~810px
- [x] MAJOR Severity mark is a bare coloured dot, not an icon
- [x] MINOR Dot rows carry no labels
- [x] MINOR Warning strip muddier and heavier than the red refusal above it

### loading
- [x] BLOCKER 
- [x] BLOCKER  (orange crescent outside the circle)
- [x] MAJOR  (uses 3:1 `track` fill)
- [ ] DEFERRED MAJOR RefreshVeil spinner collides with the text underneath — the chip is opaque and on its own surface now, but the exhibit cell is narrower than the sentence it veils, so it still lands mid-word
- [ ] DEFERRED MINOR State cards are flat: no secondary line, no cancel affordance — each state now carries its own mark and label; the secondary line and cancel were not added
- [x] (code)  (repeating anim clamps to frame 0, opacity ~0.08)
- [x] (code)  leaves a parked bright sliver
- [x] (code)  — reads as "retrying", not "loading"
- [x] (code) ; no Sizable; raw f32 cell_size

### visual-effects
- [x] MINOR  (gradients here are the subject, legitimate)

### cinematic-effects
- [ ] MINOR Decorative ring crosses the caption letterforms on the reduced-motion poster

### choice
- [x] MINOR Slider track flat; ticks vanish; disabled slider nearly identical to enabled

### input
- [x] MAJOR Fields have no labels and no visible boundary on dark
- [x] MAJOR Invalid field shows a red ring but no error message line
- [x] MAJOR Clear affordance stranded mid-field instead of trailing edge
- [x] MINOR Prefix/suffix adornments read as typed characters

### textarea
- [x] MINOR Leading textarea missing its label
- [x] MINOR Counter aligns to nothing

### form
- [ ] DEFERRED MAJOR Open combobox covers its own field label; first row clipped mid-glyph — the popup no longer reaches its label; the half row left at the top edge is the list scrolled to the selected option, and that region draws no edge fade of its own
- [x] MAJOR ~250px dead gap breaks the form rhythm
- [x] MINOR Stepper arrows are bare glyphs, no chips
- [x] MINOR Trailing icon button unaligned to field centre

### auth-sign-in
- [x] MINOR Bare grey dot as the info icon
- [x] MINOR Sparse: no identity field, no header block, over-padded panel

### auth-verification
- [x] MAJOR OTP entry is one flat bar with hairlines: no cells, no gaps, no focus, no filled/empty distinction
- [ ] MINOR Bare dot icon; resend/timer line has no affordance

### actions
- [x] MINOR First icon button chipless while the rest carry chips
- [x] MINOR Overflow menu: no icons, no shortcuts, destructive item not differentiated

### progress-bar
- [x] BLOCKER 
- [x] BLOCKER 
- [x] MAJOR  with a corrupted fill
- [ ] DEFERRED MAJOR Metadata alignment differs between identical rows — the first pair shares a right edge; the Stalled/Paused pair does not, because only one of them carries a Cancel control
- [x] MINOR 

### state-ladder
- [x] MAJOR  in a state-comparison scene
- [x] MAJOR Error card's red bloom bleeds into the Ready column
- [x] MAJOR Idle/Queued/Blocked and Empty/Cancelled pairs render identically
- [ ] MINOR Column headings not baseline-aligned

### banner
- [x] MAJOR "Try again" stretched to banner width — reads as a second bar
- [x] MINOR Bare dot severity marks; warning fill heavier than error

### outcome-panel
- [x] MAJOR Tone halo spills far past the card edge, loudest thing in frame
- [x] MINOR No outcome offers an action

### stage-progress
- [x] MAJOR Under-built: dot + label + floating hairline; no numbering, no connectors, no per-stage status
- [x] MAJOR Trailing rules end at ragged x positions, attached to nothing
- [x] MINOR Current stage carries no emphasis over done stages

### divider
- [x] MAJOR Near-invisible in dark (~one perceptual step)
- [x] MINOR Thin scene: no vertical, no inset variants

### tag
- [x] MINOR read-only vs pinned: two dimming levels, no explanation of which means what

### avatar
- [x] MAJOR Under-built: no image avatar, no presence dot, no group stack, no fallback icon
- [x] MAJOR Nameless avatar is an empty grey disc — reads as a failed image load
- [x] MAJOR Identity ring is a two-hue gradient
- [x] MINOR "Derived from the name" caption over three identical grey avatars
- [x] MINOR Two smaller size steps nearly the same diameter

### empty-state
- [x] MINOR Recovery actions are the dimmest element on the panel
- [x] MINOR Icon collides with state-ladder's use of the same glyph

### kbd
- [x] MINOR 

### overlay
- [x] MAJOR Scrim effectively opaque — nothing visibly "underneath"
- [x] MINOR Panel thin: title + two buttons

### dialog
- [x] MINOR No footer divider; buttons float

### tooltip
- [x] MAJOR Indistinguishable from a button; no arrow, same fill as trigger
- [x] MINOR No pointer/tail anchoring

### menu
- [x] MAJOR Submenu top-aligned to the parent panel, not to its owning row
- [x] MAJOR Two section-header styles; one reads as a disabled item
- [x] MINOR Highlight inset vs full-bleed separators disagree on margins

### context-menu
- [x] MINOR Section label reads as disabled item (same as menu)

### popover
- [x] MAJOR Trigger and panel merge into one borderless blob (worst in light)
- [ ] MINOR Placeholder-thin content

### command-palette
- [ ] MINOR Uneven row rhythm between results 2 and 3

### toast
- [x] MAJOR Action button collides with wrapped title; close x crowded
- [ ] MINOR Warning and refusal toasts tonally identical

### tabs
- [x] MAJOR 
- [x] MAJOR 
- [x] MINOR 

### accordion
- [x] MAJOR One flat slab: no item boundaries, no expanded-panel treatment
- [x] MINOR 

### breadcrumb
- [x] MINOR No hover surface or affordance; ellipsis reads as punctuation
- [ ] MINOR Variant rows nearly touch

### list
- [x] MAJOR First/last rows flush against the rounded container edges
- [ ] MINOR No divider, no metadata, no scroll hint

### flow
- [x] BLOCKER Final entry sliced horizontally mid-glyph at the panel edge
- [x] MINOR No "n more" affordance

### table
- [x] MINOR Last row flush against bottom edge
- [ ] MINOR Refusal banner and table abut with mismatched radii

### data-grid
- [x] BLOCKER Selected-row highlight is a three-tone patchwork stopping before the last column (both themes)
- [x] MAJOR Orphan "Job" label floating above the real header
- [x] MINOR Phantom empty column at right edge
- [x] MINOR Expanded detail region has no indent/rail/background
- [x] MINOR Footer summary cell unlabelled

### data-grid-editing
- [x] MAJOR Editor cell pure black and taller than its row — punches a hole in the table
- [x] MINOR Editor touches the neighbouring pill; phantom column again

### tree-grid
- [x] MAJOR States as plain text while table/data-grid use pills for the same vocabulary
- [x] MAJOR "Expanded" (structure) listed as data state
- [ ] MINOR Indent step too small, no guides; separators invisible

### tree
- [x] MAJOR Loading child identical to disabled node
- [x] MINOR No indent guides

### split-pane
- [x] MAJOR Under-built: two identical flat panes, half empty
- [x] MAJOR Drag handle is a ~24px floating stub aligned to nothing
- [x] MINOR Pane body text far below comfortable contrast

### scroll-area
- [x] MAJOR Scrollbar thumb crosses outside the rounded corner
- [x] MAJOR Bottom line clipped mid-glyph
- [x] MINOR Second panel two lines in a six-line box

### scroll-shadow
- [x] MAJOR The shadow itself is invisible in dark — scene shows only clipping
- [x] MAJOR Both edges clip glyphs with no cue
- [x] MINOR No captions

### scroll-fade
- [x] MINOR Bottom fade never demonstrated (content stops short)
- [x] MINOR Comparison confounded by very different fill ratios

### frost
- [x] MAJOR Backdrop is a raw indigo-bars test pattern reading as a hole in the panel
- [x] MAJOR 
- [x] MINOR Backdrop abuts the text column with no gutter

### glass
- [x] MAJOR Checkerboard clipped mid-square on every group edge
- [ ] DEFERRED MAJOR "Frosted" panel shows no blur while Lens/Liquid visibly distort — the tile's backdrop is nearly uniform where the panel sits, so the blur has nothing to smear; `frost` was moved over its page text and now demonstrates the same effect
- [x] MAJOR 
- [x] MINOR 

### toolbar
- [ ] MINOR Secondary Share outshouts the primary control
- [ ] MINOR Selected segment and button share one fill, two meanings

### desktop-titlebar
- [ ] MINOR Two lines cramped into ~70px; subtitle rides the divider

### sidebar
- [x] MAJOR Collapsed rail icons near-invisible (~#5a5a5a on #1a1a1a)
- [x] MINOR Rail gap implies an undrawn group divider

### pagination
- [x] MAJOR Chevron and arrow glyph styles mixed for first/prev
- [x] MAJOR Compact variant stacks both back controls together
- [x] MINOR Page-size select dominates; uneven gaps

### drawer
- [x] MAJOR ~1600px dead space between content and the pinned Apply
- [x] MAJOR Apply is a full-width near-white slab
- [x] MAJOR Scrim barely darkens the page — no modal layer
- [x] MINOR No header divider, no close, no footer separator

### motion-flip
- [x] MINOR 

### motion-state
- [x] BLOCKER 
- [x] MAJOR 

### animated-number
- [x] MAJOR Under-built: label/value pairs + button on bare canvas, reads as debug print
- [x] MINOR Stat columns packed accidentally

### drag-list
- [x] BLOCKER 
- [x] BLOCKER Drag ghost overlaps and severs the row above
- [x] MAJOR No drag affordance on resting rows

### drag-tree
- [x] MAJOR Ghost occludes the drop target row
- [x] MINOR Ghost vertically misregistered ~10px; drop highlight reads as focus ring

### dropzone
- [x] MAJOR Three tiles, three different sizes in one state row
- [x] MAJOR Idle solid ring reads as a disabled input, not a drop target
- [x] MINOR Refusing tile loses its identity line

### wizard
- [x] MAJOR 
- [x] MAJOR 
- [x] MINOR 

### undo-history
- [x] MAJOR 
- [x] MINOR 

### settings
- [x] MINOR No dividers inside cards; second card unbalanced

### detail
- [ ] MINOR Timeline connector stubs don't meet the dots
- [ ] MINOR Definition list leaves an unexplained hole

### filter-bar
- [x] MINOR "Counting…" loading is text-only, mistakable for a value
- [x] MINOR Add and Clear read at equal weight

### inline-edit
- [x] BLOCKER Double concentric red+indigo ring on the error field
- [x] MAJOR Locked state loses its field chrome entirely
- [x] MAJOR Read state has no editable affordance
- [x] MINOR 8px left-edge jog between text rows and inputs

### progress-circle
- [x] BLOCKER 
- [x] MAJOR 
- [ ] DEFERRED MAJOR Indeterminate indistinguishable from ~90% complete — the arc is a gap-bearing ring rather than a fraction, but a repeating animation is held at its first frame in a capture, so this one is judged by running the gallery, not by reading the baseline
- [x] MINOR 

### split-tree
- [x] MAJOR 
- [ ] MINOR Two handles, two placement rules; panes mostly empty

### ide-shell
- [x] MAJOR 
- [x] MAJOR 
- [x] MAJOR 
- [ ] MINOR Top-right control brightest element in the chrome

### keybinding
- [x] BLOCKER "Press a shortcut" illegible: light lavender on solid indigo
- [x] MAJOR Recording glow blooms into neighbouring rows
- [x] MAJOR "Toggle terminal" chip lost in an oversized empty field
- [x] MINOR "Not bound" styled like a value; row rhythm broken by the error row

### keymap-editor
- [x] MINOR Tertiary actions have no chrome; action stack zig-zags down the panel

### markdown
- [x] MAJOR Task-list checkboxes render as tofu rectangles
- [x] MINOR Blockquote rule same weight as code-block border

### agent-document
- [x] MINOR Generic checklist glyph as the empty-state icon

### agent-roster
- [x] MAJOR Stacked avatars slice each other's presence dots; overflow chip louder than the people
- [x] MINOR Rows have no separators; three unaligned text columns

### persona
- [x] MINOR Voice chip overlaps the avatar ring and presence dot
- [x] MINOR Voice-reactive meter is bare ticks with no track

### game-ui
- [x] BLOCKER 
- [x] MAJOR "Aegis" row has a label and no value
- [x] MAJOR Stray orange glow bleeding between reward cards
- [x] MAJOR Claim button dominates the frame
- [x] MINOR Four ability cards, four different internal layouts

### agent-run-canvas
- [x] MAJOR Node glow uncontained, cut flat at the canvas edge
- [x] MINOR Edge labels sit on the wire strokes

### agent-run-issues
- [x] MINOR One run-on sentence where a per-issue list belongs

### agent-avatar
- [x] MAJOR Waiting-for-approval and Refused render identically
- [x] MINOR Presence dot overruns the smallest size; offline dot near-invisible

### conversation
- [x] MAJOR Reaction chip renders tofu (x2)
- [x] MAJOR No message container: bare text, orphaned author-less messages
- [x] MAJOR "Sending" reused identically for queued and streaming
- [x] MINOR Status lines attach visually to the wrong message

### outline
- [x] BLOCKER Last row sliced horizontally mid-glyph in both blocks
- [x] BLOCKER Mark rail does not correspond to the rows it indexes
- [x] MAJOR Marks are loose dashes: no track, no container

### conversation-growing
- [x] MINOR "N more lines" indicator doesn't announce itself

### image-viewer
- [x] MAJOR Saturated indigo placeholder louder than any real UI
- [ ] MINOR Fit-mode selection barely visible

### transport
- [x] BLOCKER Loader gradient in both scrub bars
- [x] MAJOR Playhead ~20px ahead of its fill, grey sliver between
- [x] MAJOR Three transports, three control layouts
- [x] MINOR Mute is text in two transports, a filled button in the third

### audio-player
- [x] MAJOR Loader gradient in the scrub bar
- [x] MAJOR Playhead ahead of fill (same as transport)
- [x] MAJOR Waveform bar widths erratic — reads as rasterisation artifact
- [x] MINOR No container or centre line for the waveform

### audio-waveform
- [x] MAJOR Empty/refused states are unstyled centred text on bare canvas
- [x] MAJOR Same erratic bar widths
- [x] MINOR Checklist glyph for an audio component

### video-player
- [x] MAJOR Loader gradient in the scrub bar
- [x] MAJOR Poster crushed; two boxes crammed into one media slot
- [x] MAJOR Indigo placeholder louder than content

### model-viewer
- [x] MINOR Debug-blue cube; triangulation seam through "flat" shading
- [x] MINOR Refusal panel keeps full viewport height, 60% empty

### approval
- [x] MINOR Scope actions have no chrome — the most consequential controls look like labels
- [x] MINOR Approve visually outweighs Decline in a consent surface

### permission-matrix
- [x] MAJOR Row cells at different heights, ragged bottoms
- [x] MAJOR "Does not apply" cells drawn with no box next to bordered cells
- [x] MINOR No header separator

### cost-meter
- [x] MAJOR Estimated-against-unknown and Unavailable render identically (both bare tracks)
- [x] MINOR Stale line detaches from the row it qualifies
- [x] NOTE Fill is solid but hard-coded to accent — must become token-driven neutral per directive

### prompt-builder
- [x] MAJOR Under-built: one card + two unstyled fragments floating on canvas
- [x] MAJOR Arbitrary vertical rhythm (40px then 100px)
- [ ] MINOR Checklist glyph again; slot chips low-contrast

### feedback-rating
- [x] BLOCKER Disabled row pixel-identical to enabled row
- [x] MAJOR Crudest composition: flat pills, no icons, no grouping
- [x] MINOR Two different selected treatments in two rows

### artifact-preview
- [x] MAJOR  (gradient sampled)
- [x] MAJOR 
- [x] MINOR Card surfaces undifferentiated

### tool-call
- [x] MAJOR Six arbitrary hues on tool identifiers, uncorrelated with status
- [x] MAJOR Two-column split with a 250px dead gutter reads broken
- [x] MINOR Argument/result boxes fused at a 1px seam
- [x] MINOR "Try again" identical to the duration text beside it

### step-list
- [x] BLOCKER 
- [x] BLOCKER  — reads 100% done
- [x] MAJOR 

### node-graph
- [x] MAJOR 
- [x] MAJOR 
- [x] MAJOR Edge labels collide with wires
- [x] MAJOR Rightmost node clipped square
- [x] MAJOR Minimap is a bare rectangle with no viewport indicator
- [x] MINOR Selection halo overshoots; floating unanchored labels

### canvas-tools
- [x] MAJOR Two ~1050x230 empty tinted rectangles
- [x] MAJOR Minimap viewport drawn in alarm red, overlapping and overhanging
- [x] MINOR Zoom readout styled like the buttons around it

### browser-panel
- [x] MAJOR Loading tile is one faint word — reads stuck, not loading
- [x] MAJOR Two rows, two unrelated tile size classes
- [ ] MINOR Square borderless panels; stateless nav glyphs; inconsistent URL truncation

### thinking (quiet language kept — fixes stay inside it)
- [x] MAJOR 
- [x] MAJOR 
- [x] MINOR 

### json-view
- [x] MAJOR Zero type differentiation: strings, numbers, booleans, null one colour
- [x] MAJOR No key/value separator or column
- [x] MAJOR Selected row band ends at an arbitrary x, brightest thing in frame
- [x] MINOR No indent guides; redaction annotations float at hard-coded x

### schema-form
- [x] MAJOR 
- [x] MAJOR 
- [x] MAJOR 
- [x] MAJOR 
- [x] MINOR "Choose files" fused to dropzone; Limits child misaligned; file row chipless

### server-list
- [x] MAJOR "Turned off" body is the brightest surface in the list
- [x] MINOR Header/body seam on the expanded card; unexplained row-height differences
- [ ] NOTE Status vocabulary is the reference — do not regress

### offering-catalog
- [x] MINOR Stale banner butts the list; hover fill loudest surface; no row separators

### reading-direction
- [x] MAJOR 
- [x] MAJOR 
- [ ] DEFERRED MAJOR Sibling tree rows: three right edges, two row treatments — the selection fill defines the only real row edge; the ragged content edges are chevron and icon placement and were not chased
- [ ] MINOR Progress shown as text only; unlabelled icon strip; chevron alignment varies

### toggle
- [x] MAJOR Off state has no chrome at all — indistinguishable from a label
- [x] MAJOR Disabled and off are the same
- [x] MAJOR Segmented selection reads inverted (selected darkest)
- [x] MINOR Two corner languages across the three rows

### collapsible
- [x] MINOR No divider between header and revealed body

### hover-card
- [x] MINOR 

### menubar
- [ ] MINOR Open trigger unmarked; empty leading gutter column

### copy-button
- [x] MAJOR Idle, success, and error share one appearance
- [x] MINOR "Copied" label stutter

### aspect-ratio
- [x] MINOR Unrounded unbordered grey blocks off the house style

### responsive
- [x] MAJOR Two near-identical rectangles, one word each — uninformative
- [x] MAJOR Panels nearly invisible against canvas

### icon
- [x] MAJOR  (row 2, ~13th)
- [x] MAJOR Tone ramp unreadable: primary/muted/faint near-identical; accent == accentStrong

### document-tabs
- [ ] MINOR No strip surface or baseline rule; overflow chevron outweighs tabs

### search-field
- [x] MINOR No magnifier or clear control; chip echoes caption; kebab id shown raw
- [ ] MINOR Non-current match highlight nearly invisible

### find-replace
- [x] MINOR Field heights differ 70 vs 66px
- [x] MINOR No case/word/regex toggles, no close control

### notification-center
- [x] MINOR Unread dot butts the title text; two competing dot systems
- [x] MINOR Actions have no chrome; uneven rhythm, no dividers

### failure-panel
- [x] MAJOR Red halo bleeds ~25px past every edge
- [x] MAJOR "Try again" reads disabled — quietest control on the panel
- [x] MINOR Source label smaller than the message below it

### log-stream
- [x] MAJOR Severity pills stretched to fixed width, left-aligned labels
- [x] MAJOR ~140px dead gutter splits every row
- [ ] MINOR Current vs other search hits read as different features

### diff-view
- [x] MAJOR 
- [x] MAJOR Card widths inconsistent (full vs half)
- [x] MINOR Split view leaves silent blanks; inconsistent bottom padding

### sparkline
- [x] MAJOR Bare 2px polyline in an empty box: no fill, baseline, markers, axis
- [x] MAJOR Two different metrics draw pixel-identical curves
- [x] MAJOR Stale state invisible in the mark itself
- [x] MINOR Min/max labels stranded far from the line

### chart
- [x] MAJOR Overlapping area fills resolve to a muddy grey-brown matching no legend
- [x] MAJOR Donut is one flat circle with a hairline seam — encodes nothing
- [x] MAJOR Bottom section clipped by the frame
- [ ] MINOR Tooltip sits on the data with no leader; empty state misaligned; bars unlabelled

### metric-card
- [x] BLOCKER Loading renders as five multi-hue swatch chips flush to the frame margin
- [x] BLOCKER Loading state occupies none of the shape of the thing loading
- [x] MAJOR Empty/Unavailable/Error have no card at all, centred on bare canvas
- [x] MAJOR Card-in-card double frame
- [x] MINOR Min and Max both read 12.4k under a visibly varying line

### kanban
- [x] MAJOR Three columns, three heights, no shared baseline
- [x] MAJOR Under-built: no counts, no add, no WIP, no drop indicator
- [x] MAJOR Empty and error states escape the board entirely

### micro
- [x] BLOCKER , nothing else
- [x] MAJOR 

### trace
- [x] MAJOR 
- [x] MAJOR 
- [x] MAJOR 
- [x] MINOR 

### heatmap
- [x] MAJOR Under-built: 4x5 grid, no month/weekday labels, no legend
- [x] MAJOR Intensity steps don't separate in dark
- [x] MINOR Empty state misaligned from its heading

### color-picker
- [x] MAJOR Alpha slider has no checkerboard — nothing shows transparency
- [x] MAJOR Alpha thumb clipped to a half-circle at the track end
- [x] MINOR Alpha track height/radius differ from the hue track

### code-view
- [x] MINOR Line 44 hard-cut mid-word; highlight bands stop 37px short of the card edge
- [x] MINOR Strikethrough vs non-strikethrough red bands undistinguished; 4px gutter slivers

### upload-list
- [x] BLOCKER 
- [x] MAJOR Two cancel buttons render as blank grey squares
- [x] MINOR Overall and per-file bars don't share a left edge; no numeric progress

### cascader
- [x] MAJOR Open branch shows no child column — reads as failed to open
- [x] MAJOR 
- [x] MAJOR Trigger says one thing, highlighted row another, no check anywhere

### anchor-list
- [x] MAJOR 
- [x] MINOR No menu affordance on the overflow; bar floats containerless

### diagnostics-list
- [x] MAJOR Same four filters expressed twice in two visual languages
- [x] MAJOR All four filter pills identically "active"
- [x] MAJOR Severity badges land at four different x positions
- [x] MAJOR 
- [x] MINOR "Inspect fixture" plain text, brighter than the message

### terminal
- [x] MAJOR Loading card: five multi-hue dots centred in 830x360 of empty black
- [ ] MINOR Two caret renderings in one scene; "Session ended" reads as program output; ragged grid
- [ ] NOTE ANSI rendering itself is good — do not regress

### calendar
- [x] MAJOR Event dots push their day numbers ~6px up, breaking every dotted row
- [x] MAJOR Adjacent-month days at full weight; blocked days get the dim treatment instead
- [ ] MINOR Selected day's dot invisible on the selection fill; empty card keeps its nav arrows; card pair heights unsettled

### date-range
- [x] MAJOR In-range days render as separate chips — a range doesn't read as a range
- [x] MAJOR Blocked day inside a range indistinguishable from an ordinary in-range day
- [x] MAJOR Two different states share one identical caption
- [x] MINOR Today ring reuses the endpoint colour

### date-time
- [x] MAJOR Checklist glyph where a calendar icon belongs (both fields)
- [x] MAJOR Time steppers are static text: no arrows, no segment focus, no labels
- [x] MINOR Separator and width inconsistencies between the paired steppers

---

## Close-out

Every **BLOCKER** and **MAJOR** in this document is either ticked or carries a
`DEFERRED` line saying what was found and why it was left. No open row of
either severity remains. The rows still showing `- [ ]` without `DEFERRED` are
**MINOR** or **NOTE** and were not re-inspected in the closing pass; they are
open findings, not silent passes.

A ticked box here means one thing: the defect was looked for in the current
baseline and is not there. The loader-gradient family (root cause A) was
settled differently, and it is worth saying how, because it is the largest
group in the document. The three gradient stops no longer exist anywhere in
the tree, and every `color.loader` role is held achromatic on both themes by
`display::signature`'s `no_loader_role_carries_a_meaningful_hue`, so those rows
are closed by a test rather than by counting pixels in seventeen pictures.

Three claims raised in review were checked against the pictures and are not
defects:

- **offering-catalog** carries no gold plan tags. `tool`, `skill` and
  `resource` are neutral. The only amber is the stale-archive warning, which
  is a state.
- **kanban** column headers are neutral. The amber is the `3 of 2` WIP badge
  and the sentence under it. The dashed accent lane is the library-wide
  drop-landing mark and draws only while a card is carried.
- **hover-card** has a real shadow and a raised fill; the accent is a link and
  the green is a status. A hover card is anchored rather than pointer-tracked,
  so it takes no tail.

Two more were reclassified rather than fixed:

- **icon** — the "solid filled block" in row two is `stop`, and a stop glyph is
  a filled square. It sits beside `play` and `pause` and reads correctly.
- **thinking** — the two rows now differ in text tone, and the active row
  carries an accent mark that pulses. A repeating animation is held at its
  first frame in a capture, so the motion half of that row is reviewed by
  running the gallery.

One defect the audit never recorded was found while closing it out, and it was
the worst one left: every keycap modifier drew as a blank box. `Kbd` names a
bundled face in its fallback list, and both text systems threw that face away
for having no `m` — the character an em is measured with. A face named as a
*fallback* is never measured with, so requiring `m` of it rejected exactly the
symbol-only faces a fallback list exists to name. See the CHANGELOG.

Closing pass: 2026-08-24, against `snapshots/headless/macos/scenes` at the
capture taken after the last fix landed, both themes read wherever a contrast
or colour question came up.

## Quality score — full public catalog, 2026-08-26

This pass supersedes the current-status conclusions above without rewriting
that historical audit. It reviewed all 180 public components in
`docs/api-index.json` against their current Rust API and implementation,
declared exhibits, both macOS `studio-dark` and `studio-light` headless
baselines, and focused behavior tests. Scores use this 100-point rubric:

| Dimension | Points | Review question |
|---|---:|---|
| Truthful states and correctness (T) | 25 | Are loading, empty, unavailable, error, ready, and refresh states distinct and geometrically correct? |
| Interaction and semantics (I) | 20 | Are pointer, keyboard, focus, disabled behavior, and stable semantic targets complete? |
| Visual hierarchy, rhythm, and contrast (V) | 25 | Is the component legible, balanced, token-driven, and credible in both themes? |
| Product-neutral API and boundary (A) | 15 | Is policy caller-owned and reusable infrastructure implemented at the correct boundary? |
| Tests, exhibits, and cross-theme evidence (E) | 15 | Do focused tests and declared scenes make the contract reviewable and regression-resistant? |

The acceptance threshold is **90/100 with no blocker, major interaction
defect, or critical boundary defect**. A high total cannot compensate for one
of those failures.

### Result

| Family | Components | Average | Floor | Passing |
|---|---:|---:|---:|---:|
| Controls + datetime | 41 | 94.2 | 90 | 41 |
| Display + motion + effects | 47 | 93.8 | 91 | 47 |
| Navigation + layout + overlay | 34 | 97.1 | 93 | 34 |
| Data + structured | 11 | 96.9 | 94 | 11 |
| Content + media | 15 | 95.8 | 93 | 15 |
| Agent + game + canvas | 32 | 93.8 | 90 | 32 |
| **Catalog** | **180** | **94.9** | **90** | **180** |

The review initially found eleven real failures and one visual claim that did
not survive direct verification. Remediation completed keyboard activation for
`BulkBar`, `DataGrid`, `Table`, `JsonView`, `FeedbackRating`, `CanvasToolbar`,
and `Minimap`; corrected Minimap pointer geometry; and added focused evidence
for `AgentActivityLine`, `AgentRunIssues`, `ArtifactPreview`, and `NodeGroup`.
The `RefreshVeil` source and both baselines already put the status chip below,
not over, the verified content; a focused test now protects the retained value.

The same pass also made unknown `ProgressCircle` state visually distinct from
high determinate progress, reflowed the chart exhibit so every chart subject is
visible, completed keyboard and semantic activation for `KanbanBoard` and
`PromptBuilder`, added a product-neutral empty-state icon override, separated
standing approval scopes from immediate approval actions, and replaced
`ToolCall`'s local retry control with the standard `Button`. Direct re-review
confirmed that `AspectRatio` and `Responsive` already had adequate geometry
tests and representative exhibits. No component remains below the quality
threshold.

### Component scorecard

These are final totals under the rubric above. Family membership follows the
API index; repeated public names in different families are scored in their own
context.

**Controls + datetime (41):**

`Button` 94 · `ButtonGroup` 92 · `Cascader` 95 · `Checkbox` 94 ·
`ColorPicker` 91 · `ColorSwatch` 90 · `Combobox` 95 · `CopyButton` 94 ·
`Dropzone` 94 · `FilterBar` 93 · `FindReplace` 94 · `FormField` 95 ·
`IconButton` 94 · `InlineEdit` 94 · `KeybindingRecorder` 95 ·
`KeymapEditor` 95 · `MentionInput` 96 · `NumberInput` 95 ·
`OneTimeCodeInput` 96 · `PasswordInput` 96 · `Radio` 94 ·
`RichTextEditor` 95 · `SearchField` 95 · `SegmentedControl` 94 · `Select` 95 ·
`SettingsList` 92 · `SettingsRow` 94 · `SettingsSection` 93 · `Slider` 95 ·
`SplitButton` 94 · `Switch` 95 · `TagInput` 94 · `TextArea` 96 ·
`TextInput` 94 · `Toggle` 93 · `ToggleGroup` 95 · `UploadList` 95 ·
`Calendar` 95 · `DateInput` 94 · `RangePicker` 94 · `TimeInput` 94.

**Display + motion + effects (47):**

`AnimatedNumber` 93 · `AreaChart` 95 · `Avatar` 93 · `AvatarGroup` 92 ·
`Badge` 94 · `Banner` 94 · `BarChart` 93 · `BarLoader` 91 · `Callout` 93 ·
`Card` 94 · `ChartLegend` 94 · `DescriptionList` 94 · `Divider` 93 ·
`EmptyState` 97 · `FailurePanel` 96 · `GaugeChart` 92 · `Heatmap` 92 ·
`HighlightedText` 94 · `Icon` 96 · `LineChart` 97 · `ListRow` 94 ·
`LoadMore` 94 · `MetricCard` 94 · `OutcomePanel` 95 · `PieChart` 93 ·
`ProgressBar` 96 · `ProgressCircle` 96 · `PulseLoader` 91 · `RadarChart` 92 ·
`RefreshVeil` 94 · `ScatterChart` 95 · `Skeleton` 92 · `SpanTimeline` 93 ·
`Sparkline` 96 · `Spinner` 91 · `StackedBarChart` 93 · `StageProgress` 94 ·
`StaleMark` 94 · `StateView` 96 · `StatusDot` 92 · `StatusLine` 93 ·
`Tag` 95 · `Timeline` 94 · `TraceView` 94 · `CinematicEffect` 94 ·
`EffectParticles` 95 · `MicroMark` 91.

**Navigation + layout + overlay (34):**

`Accordion` 94 · `AnchorList` 97 · `Breadcrumb` 94 · `Collapsible` 97 ·
`Pagination` 98 · `Sidebar` 96 · `Tabs` 98 · `UndoHistory` 98 · `Wizard` 98 ·
`AspectRatio` 93 · `DesktopTitlebar` 97 · `Dock` 98 · `Responsive` 95 ·
`ScrollArea` 98 · `ScrollFade` 94 · `SplitPane` 98 · `SplitTree` 98 ·
`StatusBar` 97 · `Toolbar` 97 · `CommandPalette` 98 · `ContextMenu` 98 ·
`Dialog` 99 · `Drawer` 98 · `Frost` 95 · `Glass` 99 · `HoverCard` 98 ·
`Kbd` 98 · `Menu` 98 · `Menubar` 98 · `NotificationCenter` 98 ·
`Overlay` 98 · `Popover` 97 · `ToastLayer` 98 · `Tooltip` 98.

**Data + structured (11):**

`BulkBar` 96 · `DataGrid` 98 · `DiagnosticsList` 96 · `Flow` 96 ·
`KanbanBoard` 97 · `List` 94 · `Table` 98 · `Tree` 98 · `TreeGrid` 96 ·
`JsonView` 99 · `SchemaForm` 98.

**Content + media (15):**

`AgentDocument` 96 · `BrowserPanel` 96 · `CodeView` 94 · `DiffView` 98 ·
`ImageViewer` 96 · `LogStream` 97 · `Markdown` 97 · `MessageList` 97 ·
`Outline` 93 · `Terminal` 96 · `TransportBar` 98 · `AudioPlayer` 96 ·
`AudioWaveform` 93 · `ModelViewer` 97 · `VideoPlayer` 93.

**Agent + game + canvas (32):**

`AgentActivityLine` 94 · `AgentAvatar` 93 · `AgentCard` 92 · `AgentGroup` 90 ·
`AgentRoster` 94 · `AgentRunCanvas` 95 · `AgentRunIssues` 93 ·
`ApprovalPrompt` 95 · `ArtifactPreview` 95 · `ContextGauge` 94 ·
`CostMeter` 94 · `FeedbackRating` 96 · `OfferingCatalog` 94 ·
`PermissionMatrix` 95 · `PersonaDialogue` 93 · `PersonaPortrait` 92 ·
`PromptBuilder` 92 · `ServerList` 95 · `StepList` 94 · `SubagentTree` 93 ·
`ThinkingBlock` 94 · `ToolCall` 96 · `VoiceReactive` 91 · `AbilityBar` 93 ·
`ObjectiveTracker` 94 · `PartyRoster` 93 · `RewardReveal` 92 ·
`CanvasToolbar` 96 · `GraphNode` 94 · `Minimap` 96 · `NodeGraph` 96 ·
`NodeGroup` 94.

## UX re-review — visual finish and component completeness, 2026-08-26

The score above answers whether a component is correct, testable, and built at
the right boundary. This second pass asks a different question: whether a user
can understand and trust the thing on screen, and whether the public component
is complete enough for a credible product use. Passing the first rubric does
not imply passing this one.

All 180 public components were reviewed by family against their implementation,
declared exhibit, and both current macOS themes. Motion-only conclusions were
not inferred from still captures: those components retain their gallery review
requirement. The UX score is a review judgement, not a test metric:

| Dimension | Points | Review question |
|---|---:|---|
| Visual completion | 35 | Does hierarchy, density, spacing, contrast, and state emphasis look intentional in both themes? |
| Component completeness | 30 | Does the public surface cover the states and scale a real host needs without a local imitation? |
| Usability | 20 | Can a reader discover actions, scan identity, and understand disabled/refused/unknown states? |
| Exhibit credibility | 15 | Does the review scene exercise believable content rather than hiding behind idealized placeholders? |

Acceptance remains **90/100 with no major UX defect**.

### Current UX score

| Family | Components | Average | Floor | Passing |
|---|---:|---:|---:|---:|
| Controls + datetime | 41 | 94 | 91 | 41 |
| Display + motion + effects | 47 | 93 | 90 | 47 |
| Navigation + layout + overlay | 34 | 93 | 90 | 34 |
| Data + structured | 11 | 91 | 90 | 11 |
| Content + media | 15 | 94 | 91 | 15 |
| Agent + game + canvas | 32 | 92 | 90 | 32 |
| **Catalog** | **180** | **93** | **90** | **180** |

### Findings closed in this pass

- `ColorPicker` now names the current colour and visibly reports saturation /
  brightness, hue, and opacity. `ColorSwatch` selection and disabled treatment
  survive both themes, and the exhibit no longer selects the same colour in
  both host lists.
- `RangePicker` no longer loses the third example beyond the review frame. A
  blocked day keeps the continuous range underneath it and carries a warning
  edge as well as the blocked numeral and exact host reason.
- `RadarChart` puts axis labels at their axes instead of collecting them in an
  unrelated row. `GaugeChart` puts its reading inside the open centre of its
  scale instead of below a detached canvas.
- `ImageViewer` and `VideoPlayer` now receive deterministic but credible host
  content. A supplied poster is presented as a neutral fallback rather than a
  warning, while a genuinely empty video surface still says no frame exists.
- `AbilityBar` is compact and keeps unavailable identities readable without
  installing actions. `RewardReveal` gives the celebration its own bounded
  visual address, keeps it off copy, and supplies a neutral fallback glyph for
  a reward item whose host supplies no art.
- `MinimapMark` can carry the same caller-owned category colour as a graph
  node. The canvas exhibit maps all four marks, fixes the clipped caption, and
  removes the unused minimap column width.

Each changed exhibit was re-captured and read in `studio-dark` and
`studio-light`. The final pictures contain no text clipping, overlap, missing
glyph, or low-contrast state in the changed subjects.

### Wide-data completeness gap closed

`DataGrid` now scores **96** and `TreeGrid` **95**. A single horizontal
viewport carries header, virtualized body, and summary while vertical
virtualization remains independent. The leading column group stays frozen at
the left edge in LTR and the right edge in RTL; pointer geometry, clipping,
accessibility bounds, and focus reveal all use the same translated subtree.

This is backed by a product-neutral GPUI sticky primitive rather than a
body-only wrapper or per-row counter-translation. Focused tests cover shared
header/body/summary motion, frozen hierarchy, reserved-edge keyboard reveal,
LTR/RTL direction switches, clipped hit targets, and translated accessibility
bounds. The wide exhibits name the offscreen fields and make scale behavior
reviewable instead of hiding it behind short idealized columns.

All six affected `data-grid`, `data-grid-editing`, and `tree-grid` baselines
were re-captured and inspected in `studio-dark` and `studio-light`. The review
found and corrected one additional hierarchy-header gutter mismatch; the final
images have aligned header/body geometry, clear frozen casts, legible selected
and editing states, and no unintended clipping, overlap, or missing glyph.

### Claims rejected or reduced after direct review

- `AgentRoster` does show its subagent hierarchy; its remaining sparsity is an
  exhibit-density opportunity, not a missing tree.
- `PersonaPortrait` / `PersonaDialogue` already show portrait states, choices,
  a disabled reason, voice-reactive state, and unavailable state. Their fixture
  nature is visible but the components are not incomplete.
- `Terminal` has clear rendering and state vocabulary; the large quiet region
  is a sparse fixture, not a terminal rendering defect.
- Spinner, pulse, bar-loader, micro-mark, and thinking motion cannot be failed
  from a reduced-motion poster. Their user experience remains a real-gallery
  review, as documented by the visual testing contract.
- The earlier short `DataGrid` and `TreeGrid` fixtures were legible, but did not
  establish wide-content completeness. Their replacements now expose that
  behavior directly and retain the same clear short-content states.
