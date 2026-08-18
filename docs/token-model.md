# Token model

## Authority

The documents under `crates/gpui-kit-tokens/tokens/` are the authority, and
they sit inside the crate that embeds them so that crate can be packaged on its
own. `schema.json` is the single current portable schema; documents and nested
objects reject unknown or legacy fields rather than carrying compatibility
readers. Cargo package `gpui-box-kit-tokens` embeds and validates every bundled
theme; `gpui-box-kit-theme` is the only GPUI adapter.
`studio-dark` is the default and `studio-light` is its light counterpart.

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
popover.border = interactive.hairlineStrong
popover.radius = radius.card
```

Component aliases are implemented in Rust while the catalog is small. Move an
alias into JSON only when several components must share and evolve it together.

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

### Surface separation

A contrast ratio answers whether a foreground is legible on a background. It
does not answer whether two backgrounds are distinguishable, and the two
questions need different measures. The WCAG ratio adds 0.05 to both sides so
that black text stays measurable, which compresses everything near black into
almost no range: `#050505` behind `#0a0a0a` reports 1.03:1, and so does a step
a reader can point at. Near white it fails the other way, by reporting a
comfortable-looking number for a step nobody can see.

So surfaces are compared in CIE L\*, which is uniform across the range, and
every nesting two surfaces can form must gain at least **3 L\*** over the one
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
sits on the substrate, it sits in a panel or on the page, and requiring
three L\* between those two would collapse the dark ramp. `overlay` is
checked against what it opens over rather than against `raised`, because a
popover and a code block never touch.

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

## Elevation, layers, and density

`elevation` describes the shadow each surface casts. A step is an ordered
set of layers, not a single offset: `flat` is empty, and `raised`,
`overlay` and `modal` may each carry more than one downward cast. There is
no horizontal offset; a close contact shadow is `y` plus `blur`. Steps are
ordered by reach — the farthest `y + blur` in the set — and
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

Static Medium, SemiBold, and Bold font files are included because not every
GPUI text backend applies a variable font's weight axis.

## Motion and effects

Motion tokens store duration and cubic-bezier control points. Component motion
evaluates CSS-compatible curves through the pure `CubicBezier` implementation.

Overlay surfaces are opaque by default. `Frost` is the one surface that is not:
`effect.glassAlpha` is how opaque its own fill is over what it blurs and
`effect.glassBlur` is how far that blur reaches. A theme that sets the alpha to
1 declares itself opaque and no blur is painted at all, and where the renderer
provides no backdrop blur the tinted fill is drawn on its own — GPUI Box Kit does
not fake a blur with a gradient, because the colour behind a translucent window
is not a colour anything can paint.

The two rings live here too. `effect.selectedRingAlpha` draws the inset ring on
the current answer; `effect.focusRingWidth` and `effect.focusRingAlpha` draw the
outset ring, in `color.interactive.focus`, on whatever holds the keyboard. They
are separate tokens because they answer separate questions, and a theme that
collapsed them would leave a reader unable to tell selection from focus.

## Validation

`TokenDocument::validate` rejects:

- a missing schema declaration or unknown fields at any level;
- invalid RGB/RGBA literals and unresolvable palette references;
- empty metadata;
- negative geometry and non-increasing spacing or control heights;
- invalid type size, line-height, or weight;
- effect and opacity alpha outside 0–1, or a non-positive focus ring width;
- negative elevation blur, or elevation steps whose reach (`y + blur` of
  the farthest layer) is not strictly increasing;
- z-index layers that are not strictly increasing;
- density factors outside 0.5–1.5, or a `comfortable` axis that is not 1.
- required foreground/background pairs below their contrast floor.

Run:

```bash
cargo run -p xtask -- tokens generate
cargo run -p xtask -- tokens check
```
