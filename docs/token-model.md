# Token model

## Authority

The documents under `crates/gpui-kit-tokens/tokens/` are the authority, and
they sit inside the crate that embeds them so that crate can be packaged on its
own. `schema.json` is the single current portable schema; documents and nested
objects reject unknown or legacy fields rather than carrying compatibility
readers. Cargo package `gpui-box-kit-tokens` embeds and validates every bundled
theme; `gpui-box-kit-theme` is the only GPUI adapter.
`studio-dark` is the default and `studio-light` is its light counterpart.

Eight further documents ship beside them — `catppuccin-mocha`,
`catppuccin-latte`, `nord`, `tokyo-night`, `gruvbox-dark`, `dracula`,
`solarized-dark` and `solarized-light` — with palettes transcribed from their
upstream schemes (`PROVENANCE.md` P11). They carry the studio pair's exact key
set, are validated by the same gates, and `ThemeRegistry::new` registers them
after it. They are deliberately not part of `tokens::bundled()`: that pair is
what the library designs against and captures its visual baselines in, and
every scene is rendered once per member of it. `tokens::all()` is the whole
shipped catalog, and `tokens::presets()` is the eight on their own.

```text
JSON token document
    ↓
typed semantic roles
    ↓
GPUI Theme global
    ↓
component recipes
    ↓
application views
```

No crate reads a path outside this repository.

## Three layers

### Raw values

Literal values live only in the token document, in one palette per theme:

```json
"palette": { "neutral": { "200": "#1b1b1b" } }
```

Everything else references the palette, optionally with a hexadecimal alpha
suffix, so a theme is retuned by editing its scales rather than every role:

```json
"raised": "{neutral.200}",
"hover": "{neutral.850}/24"
```

### Semantic roles

Callers select meaning:

```rust
tokens.surface(Surface::Raised)
tokens.semantic(SemanticColor::Danger)
tokens.radius(Radius::Dialog)
```

### Component usage

Components combine semantic roles:

```text
popover.background = surface.overlay
popover.separator = interactive.divider
popover.radius = radius.card
```

Component aliases are implemented in Rust while the catalog is small. Move an
alias into JSON only when several components must share and evolve it together.

An alias names a complete entity recipe, not only one primitive value. For
example, `OverlaySurface::FLOATING` pairs `radius.card` with
`elevation.overlay`, `OverlaySurface::MODAL` pairs `radius.dialog` with
`elevation.modal`, and `OverlaySurface::EDGE` keeps the modal elevation but no
radius because a drawer is attached to the window plane. Message surfaces read
`radius.bubble`. This is what prevents two components with the same elevation
from accidentally claiming the same shape.

Palette-backed variants follow `color.paletteSteps`: ordered filled, hover,
active, and per-appearance readable fallback lists. The theme adapter resolves
the first step the selected palette group declares. This selection policy is
part of the theme document rather than a hard-coded Open Color convention in
Rust, so a sparse or differently numbered custom palette can retune variants
without replacing the resolver.

### Ownership boundary

Configurability depends on where a value is owned, not on whether it has a
descriptive local variable name. Production code follows this boundary:

| Owner | Values that belong there |
|---|---|
| JSON tokens | palette and semantic colours; reusable alpha ladders and effects; spacing; entity radii; typography; control scale; cross-component measures; elevation; layer order; opacity roles; shared motion duration, easing, spring, and response |
| Theme adapter | typed access to one token, and complete product-neutral recipes composed only from tokens, such as a semantic wash, border strength, variant colour set, focus ring, or overlay surface |
| Component | caller-owned data; transient interaction state; composition of shared recipes; one component's named layout topology, hit-test geometry, asset geometry, data visualization encoding, or animation keyframe shape |
| Scene or test | fixture values and assertion geometry, which are deliberately not production style authority |

A value moves into JSON when changing it should retune more than one component
or one shipped theme. A one-off width can stay local because it describes a
component's topology; a readable-copy width cannot, because empty, error, media,
and dock surfaces all make the same reading decision. Likewise, zero and one
may stay as mathematical endpoints of a gradient or interpolation, but the
alpha of a semantic wash belongs to a token even if only one call site happened
to expose the omission first.

`cargo run -p xtask -- tokens check` enforces the consumption side with a Rust
AST pass over `gpui-kit-theme` and production `gpui-kit` source. It rejects
anonymous non-zero spacing, radius, typography, and font-weight literals;
literal alpha in theme recipes; literal alpha on theme colour roles; and
anonymous non-endpoint colour alpha in components. Local visualization policy
must therefore be named, while shared appearance policy must be token-backed.
Scenes and test-only items are excluded because they are consumers and
fixtures, not authorities.

## Color

Colors use `#RRGGBB`, `#RRGGBBAA`, or a `{group.step}` palette reference. Alpha
is part of a token where it expresses a reusable wash, hairline, or effect.
Application views do not invent new palette colors.

Large planes use surface roles. Accent and semantic colors remain compact.

### Contrast

`TokenDocument::parse`, `TokenDocument::validate`, and therefore
`ThemeRegistry::register_json` reject a theme that drops below its contrast
floor. The error names every failing foreground/background pair, its measured
ratio, and its required ratio. The floors are 4.5:1 for body text and
`text.onAccent` over `semantic.accent`, and 3.0:1 for `text.faint` and status
colors, which never carry required instructions on their own.
`semantic.accentStrong` is an emphasis, border and hover color rather than a
text-bearing fill, so it is held to the non-text minimum. `cargo run -p xtask --
tokens check` applies the same contract to the bundled documents.

### Lines a pointer acts on, and lines that only divide

The 3:1 non-text floor asks whether a reader can find a boundary they have to
aim at. It is the right question for `interactive.track` and
`interactive.hairlineStrong` — a slider rail, a switch edge, a scrollbar gutter,
a resize seam — and those two are checked against every surface.

It is the wrong question for `interactive.hairline` and `interactive.divider`.
Nobody aims at a rule between two menu groups; it only has to be seen without
being noticed. Holding them to 3:1 against the darkest and the lightest surface
in the theme forces alphas in the 35–55% range, and a line that strong drawn
around every card, table header, menu, and toolbar is exactly the boxed-in look
a borderless library exists to avoid. That was not theoretical either: it is
what `studio-dark` and `studio-light` shipped, at `hairline` 35% and `divider`
40%, and every scene in the catalog wore a grid.

So the two roles carry a different gate. Each is composited over each of the six
surfaces, and the result must stand at least **1.5 L\*** away from the surface it
was drawn on:

| Role | Gate | Question it answers |
|---|---|---|
| `track`, `hairlineStrong` | 3:1 contrast ratio | can a reader aim at it? |
| `hairline`, `divider` | 1.5 L\* separation | can a reader see it at all? |

The floor is a floor, not a target. A line that clears 3:1 passes it too, which
is why the library's own test asserts the reverse direction as well: the bundled
themes must keep their decorative lines *below* the control-boundary minimum.

`TokenError::Line` reports a violation, naming the role, the surface it
disappeared against, and its measured separation. `contrast::line_report`
returns the full table.

### Surface separation

A contrast ratio answers whether a foreground is legible on a background. It
does not answer whether two backgrounds are distinguishable, and the two
questions need different measures. The WCAG ratio adds 0.05 to both sides so
that black text stays measurable, which compresses everything near black into
almost no range: `#050505` behind `#0a0a0a` reports 1.03:1, and so does a step
a reader can point at. Near white it fails the other way, by reporting a
comfortable-looking number for a step nobody can see.

So surfaces are compared in CIE L\*, which is uniform across the range, and
every nesting two surfaces can form must gain at least **2 L\*** over the one
behind it:

| Surface | Behind |
|---|---|
| `canvas` | above `backdrop` |
| `panel` | above `backdrop`, and above `canvas` |
| `sunken` | below `canvas`, and below `panel` |
| `raised` | above `panel` |
| `overlay` | above `panel`, and above `canvas` |

The ramp climbs away from the page in **both** appearances: a well is below
what holds it, a panel is above the page, and a block inside a panel is above
the panel. A light theme therefore does not paint the page white and leave
nothing above it. The page is tinted, and white is what the ramp climbs to,
which is how a native window already separates its background from its
content. `backdrop` is the substrate behind the page, the plane a card can
sit on when the page itself is not dark enough to carry the shadow. It is
checked against `canvas` and `panel` and not against `sunken`: a well never
sits on the substrate, it sits in a panel or on the page, and holding those
two apart as well would collapse the dark ramp. `overlay` is
checked against what it opens over rather than against `raised`, because a
popover and a code block never touch.

Two is a floor and not a ladder. Five rungs sit under one white ceiling, so
every tenth added to this number is taken off the page: hold the step at three
and the darkest plane of a light theme has to fall past 90 L\*, which is
below where shipped chrome sits — a native window and the editors this
library is compared to step about 2.4 L\* from their brightest plane to the
one under it. A theme is free to spread its ramp wider; what the rule refuses
is two planes a reader cannot tell apart at all.

This rule exists because its absence was not theoretical. `studio-light` gave
`panel`, `raised` and `overlay` the same `#ffffff`: a card, the code block
inside it, and the popover over it were one undivided field of color, every
contrast pair passed, and nothing in the build said so. `studio-dark` crammed
its whole ramp between `#050505` and `#242424`, so a card sat on the page with
1.08:1 between them and a black shadow that a near-black page absorbed.

`ThemeRegistry::register_json` rejects a violation with `TokenError::Separation`,
naming each nesting, its measured distance and its minimum.

### Tone distinction

The same argument applies to the foreground, and the same measure answers it.
`muted`, `faint`, `placeholder` and `disabled` are four different facts, not
four intensities of one: information that is secondary, detail that supports
it, a description of a value that is **not there**, and a value that is there
and **cannot be used**. A theme that gives two of them one colour has not
styled them alike, it has stopped saying which one holds, and this library's
own rule is that unavailable and absent are distinct states rather than
degrees of the same one.

Each rung must therefore stand at least **3 L\*** closer to the page than the
one above it:

| Tone | Stands closer to the page than |
|---|---|
| `muted` | `primary` |
| `faint` | `muted` |
| `placeholder` | `faint` |
| `disabled` | `placeholder` |

Distance from the page, rather than lightness, is what makes one rule hold in
both appearances: a dimmer fact is darker in a light theme and lighter in a
dark one, and both are the same movement toward the canvas.

This rule also exists because its absence was not theoretical. `studio-dark`
drew `faint`, `placeholder` and `disabled` in one grey. Every contrast pair
passed — all three were perfectly legible — and eight reviewers reported
across a dozen scenes that unavailable values were unreadable. They were not
unreadable. They were indistinguishable, which the contrast table had no way
to say.

`TokenError::Distinction` reports a violation, naming each pair, its measured
distance and its minimum.

### The series scale

`color.sequence.categorical` is an ordered list of exactly eight colors, and
the length is part of the contract: `TokenDocument::validate` rejects any
other count, and a caller reading past the end wraps rather than running out.

It exists because a chart with no categorical scale draws its series in the
only thing it has, which is one hue at four lightnesses — and that tells a
reader the four slices of a donut are four degrees of one quantity. The scale
separates them by hue instead: indigo, teal, amber, pink, cyan, lime, violet,
orange. Dark themes take a lighter step of each ramp and light themes a
darker one, and every entry is held to the 3:1 identity floor on each of the
three surfaces a chart is drawn on — `canvas`, `panel` and `raised`. Not
`backdrop`: that is the substrate *behind* the page, no plot is drawn on it,
and holding a light theme's scale to it would darken eight colours for a
surface nobody uses.

### The node canvas

`color.node.*` is the vocabulary a graph is read in. Every value in it exists
because a canvas drawn from the control vocabulary alone says nothing: with
one grey for every port and one for every edge, a reader cannot tell what is
attached to what without following each line by eye.

| Role | What it says | Gate |
|---|---|---|
| `portIdle` | nothing is attached here | 1.5 L\* on `canvas` |
| `portConnected` | an edge lands here | 3:1 on `canvas`, and 3 L\* further from the page than `portIdle` |
| `edge` | a resting connection | 1.5 L\* on `canvas` |
| `edgeActive` | traffic, or the pointer | 3:1 on `canvas` |
| `grid`, `gridStrong` | the dot grid and its major interval | 1.5 L\* on `canvas` |
| `labelWash` | the chip under an edge label | `text.muted` at 4.5:1 on it, over bare canvas **and** over an edge |
| `headerWash` | a node's header band, uncategorised | 1.5 L\* on `raised` |

The two floors are the ones the rest of the document already uses, and they
are assigned by the same question. A canvas is mostly edges and grid, so
drawing either at a control boundary's loudness turns a graph into a mesh:
those carry the line rule, which only asks whether they were drawn at all. A
connected port and a live edge are what a reader scans for, so they carry the
identity floor. And the port pair carries the tone ladder's rule as well,
signed the same way, because *attached* is the louder fact in both
appearances.

`labelWash` is checked twice on purpose. A chip that is legible on the bare
canvas and too thin to cover the line running under it is exactly the label
nobody can read, and the first check passes it happily.

## Elevation, layers, and density

`elevation` describes the shadow each surface casts. A step is an ordered
set of layers, not a single offset: `flat` is empty, and `raised`,
`overlay` and `modal` each carry two downward casts. There is
no horizontal offset; a close contact shadow is `y` plus `blur`.

The two layers are an **ambient** contact shadow and a **key**. The ambient
layer is tight, close and about 60% of the key's alpha; the key is the soft,
further cast that was already there. A single cast puts a surface at one
distance from the page in every direction at once, which is why one-layer
elevation reads as a sticker printed on the page rather than as a thing above
it — the contact shadow is what says the surface touches something. The
ambient layer is deliberately inside the key's reach, so the ordering of the
four steps is unchanged: reach is still the farthest `y + blur` in the set,
and that is still the key's.

Steps are ordered by reach — the farthest `y + blur` in the set — and
`TokenDocument::validate` requires that reach to increase strictly from
`flat` to `modal`. `zIndex` fixes the paint order of floating surfaces, and
`density` scales spacing, control geometry and type independently. Density
is applied when a `Theme` is built, and `gpui_kit::set_density` rebuilds the
active theme and repaints every window. Colors and radii never change with
density.

## Themes at runtime

`ThemeRegistry` holds every registered document. An application registers its
own JSON with `ThemeRegistry::register_json`, replacing a bundled theme when it
reuses its id, and switches with `gpui_kit::activate_theme`.

## Typography

The theme provides Geist and Geist Mono. Kit type styles carry bundled Noto Sans
Arabic and Noto Sans Hebrew as an explicit ordered fallback chain rather than
depending on fonts installed by the host. Size, line-height, and weight travel
as one `TypeStep`; consumers should not mix a size from one step with
line-height from another.

`typography.readoutScale` is the one intentional step beyond that prose ladder.
An `AnimatedNumber` still lets its caller choose the base `TypeScale`, then
multiplies size and line-height by this theme-owned factor so a numeric readout
does not turn `Title` into a globally larger heading style.

Static Medium, SemiBold, and Bold font files are included because not every
GPUI text backend applies a variable font's weight axis.

## Motion and effects

Motion tokens store duration and cubic-bezier control points. Component motion
evaluates CSS-compatible curves through the pure `CubicBezier` implementation.
`motion.staggerMaxItems` and `durationMs.staggerStep` jointly bound a row wave;
the public `Stagger::rows` preset takes the theme explicitly so the token is
not bypassed by a default constructor. Micro bounce, wobble, and pop timings
also live in `durationMs`; their keyframe shapes remain local component
topology.

Overlay surfaces are opaque by default. Glass separates scattering from optics:
`Frosted` uses `effect.glassFrostBlur`, while `Liquid` and `Lens` default to a
sharp snapshot (`blur = 0`) and still refract it. A caller can add blur to either
clear preset, in which case the renderer keeps both snapshots: blurred for the
interior and sharp for the refracted rim. `effect.glassAlpha` is Frosted's
source-over fill and the fill an explicitly adaptive Liquid surface may add
for readability. Ordinary Liquid and Lens paint no source-over fill: their
light response is shader-owned, where `glassTransmissionGain` multiplies the
transmitted backdrop and `glassOpticalLift` adds white light rather than
tinting over it.

The optical profile scales from `glassBevelRatio` times each control's short
edge and is bounded by `glassBevelMin`/`glassBevelMax`. Refraction and dispersion
are independent ratios; `glassHairline` remains one logical pixel rather than
growing with the control. A theme that sets `glassAlpha` to 1 declares the
surface opaque and no backdrop work is painted. GPUI Box Kit does not fake
optics with a gradient, because the colour behind a translucent window is not
a colour anything can paint.

Selection and focus live here too, and they are drawn differently on purpose:
selection says which answer is current, focus says where the next keystroke
goes, and a reader who cannot tell them apart cannot tell what pressing a key
would do. `effect.focusRingWidth` and `effect.focusRingAlpha` draw an outset
ring in `color.interactive.focus` around whatever holds the keyboard.
`effect.selectionRailWidth` is the accent bar a collection puts at the reading
edge of the row it is on, over a wash of `color.interactive.selected`.
`effect.selectedRingAlpha` remains the weight of a state tint — a changed line
in a diff, a matched range in a search, a drop target — rather than a ring drawn
around a chosen thing.

Gradients are not a token group. This library composes one from a base colour
and an alpha ladder, the way `Theme::glow` composes a bloom from
`color` plus `effect.glowAlpha`, `glowBlur` and `glowSpread`, so what a theme
owns is the scalars:

| Token | What it sets |
|---|---|
| `effect.sheenAlpha` | the strength of a top-edge highlight on a raised surface |
| `effect.areaWashAlpha` | the alpha an area fill starts at under a chart line, fading to nothing at the baseline |
| `effect.headerTintAlpha` | how strongly a node header band takes its category colour |
| `effect.nodeActive*Alpha`, `nodeTrafficAlpha`, `nodePreviewAlpha`, `nodeMinimapAlpha` | the node-canvas paint ladder for active routes, traffic, connection previews, and minimap identity |
| `effect.semanticWash*Alpha` | reusable weak, normal, and strong semantic-colour backgrounds |
| `effect.semanticBorderAlpha`, `accentBorder*Alpha` | report, selected, and active-target boundary strengths |
| `effect.variant*Alpha` | the shared Light, Outline, and Subtle state ladders |
| `effect.railWidth` | how wide an identity rail is, in pixels |
| `effect.customColorReadable*`, `customColor*LightnessDelta` | fallback readability and interaction ladders for caller colours that have no authored palette ramp |

Surface sheen, chart area wash, and node header tint are lower in light themes
than in dark ones. A wash or a highlight is read as a departure from the
surface under it, and a light theme's surfaces sit near the top of the range:
the same alpha that lights a dark card's edge has nowhere to go on a near-white
one and reads as a smudge, and the same wash under a chart line swamps the line
it is meant to support.

`effect.railWidth` is a different thing from `effect.selectionRailWidth`, and
both exist. A selection rail reports which row the collection is on and is
gone the moment another row is chosen; an identity rail says what a thing *is*
— a node's category, a callout's severity — and is drawn whether or not
anybody is looking at it, which is why it is the wider of the two.

## Validation

`TokenDocument::validate` rejects:

- a missing schema declaration or unknown fields at any level;
- invalid RGB/RGBA literals and unresolvable palette references;
- empty metadata;
- negative geometry and non-increasing spacing or control heights;
- invalid type size, line-height, weight, or a readout scale below one;
- effect and opacity alpha outside 0–1, or a non-positive focus ring width;
- crossed custom-colour readability or interaction ladders;
- negative elevation blur, or elevation steps whose reach (`y + blur` of
  the farthest layer) is not strictly increasing;
- z-index layers that are not strictly increasing;
- density factors outside 0.5–1.5, or a `comfortable` axis that is not 1;
- a non-positive selection rail width, or a non-positive identity rail width;
- a series scale that does not carry exactly eight colors;
- required foreground/background pairs below their contrast floor;
- a decorative line, a canvas grid or a resting edge that composites into a
  surface it is drawn on;
- an idle and a connected port a reader cannot tell apart.

Run:

```bash
cargo run -p xtask -- tokens generate
cargo run -p xtask -- tokens check
```
