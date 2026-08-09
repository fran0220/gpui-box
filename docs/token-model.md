# Token model

## Authority

The documents under `crates/gpui-kit-tokens/tokens/` are the authority, and
they sit inside the crate that embeds them so that crate can be packaged on its
own. `schema.json` is the single current portable schema; documents and nested
objects reject unknown or legacy fields rather than carrying compatibility
readers. `gpui-kit-tokens` embeds and validates every bundled theme;
`gpui-kit-theme` is the only GPUI adapter.
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

`cargo run -p xtask -- tokens check` fails when a theme drops below its
contrast floor: 4.5:1 for body text and `text.onAccent` over `semantic.accent`,
3.0:1 for `text.faint` and for status colors, which never carry required
instructions on their own. `semantic.accentStrong` is an emphasis, border and
hover color rather than a text-bearing fill, so it is held to the non-text
minimum.

## Elevation, layers, and density

`elevation` describes the shadow each surface casts, `zIndex` fixes the paint
order of floating surfaces, and `density` scales spacing, control geometry and
type independently. Density is applied when a `Theme` is built, and
`gpui_kit::set_density` rebuilds the active theme and repaints every window.
Colors and radii never change with density.

## Themes at runtime

`ThemeRegistry` holds every registered document. An application registers its
own JSON with `ThemeRegistry::register_json`, replacing a bundled theme when it
reuses its id, and switches with `gpui_kit::activate_theme`.

## Typography

The theme provides Geist and Geist Mono with platform fallbacks. Size,
line-height, and weight travel as one `TypeStep`; consumers should not mix a
size from one step with line-height from another.

Static Medium, SemiBold, and Bold font files are included because not every
GPUI text backend applies a variable font's weight axis.

## Motion and effects

Motion tokens store duration and cubic-bezier control points. Component motion
evaluates CSS-compatible curves through the pure `CubicBezier` implementation.

Overlay surfaces are opaque on every platform; GPUI has no per-element
backdrop blur, and gpui-kit does not fake one.

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
- negative elevation blur;
- z-index layers that are not strictly increasing;
- density factors outside 0.5–1.5, or a `comfortable` axis that is not 1.

Run:

```bash
cargo run -p xtask -- tokens generate
cargo run -p xtask -- tokens check
```
