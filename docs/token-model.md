# Token model

## Authority

`tokens/studio-dark.json` is the authority. `gpui-kit-tokens` embeds and
validates it; `gpui-kit-theme` is the only GPUI adapter.

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

Literal values live only in the token document:

```json
"raised": "#1b1b1b"
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

Colors use `#RRGGBB` or `#RRGGBBAA`. Alpha is part of a token where it expresses
a reusable wash, hairline, or effect. Application views do not invent new
palette colors.

Large planes use surface roles. Accent and semantic colors remain compact.

## Typography

The theme provides Geist and Geist Mono with platform fallbacks. Size,
line-height, and weight travel as one `TypeStep`; consumers should not mix a
size from one step with line-height from another.

Static Medium, SemiBold, and Bold font files are included because not every
GPUI text backend applies a variable font's weight axis.

## Motion and effects

Motion tokens store duration and cubic-bezier control points. Component motion
evaluates CSS-compatible curves through the pure `CubicBezier` implementation.

Glass alpha is platform-resolved by `gpui-kit-theme`: macOS may use backdrop
blur; other platforms receive opaque surfaces unless equivalent compositor
support is proven.

## Validation

`TokenDocument::validate` rejects:

- invalid RGB/RGBA literals;
- empty metadata;
- non-increasing spacing;
- invalid type size, line-height, or weight;
- effect alpha outside 0–1.

Run:

```bash
cargo run -p xtask -- tokens generate
cargo run -p xtask -- tokens check
```
