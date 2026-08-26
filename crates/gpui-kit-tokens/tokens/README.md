# Tokens

`studio-dark.json` and `studio-light.json` are the bundled semantic themes: the
pair this library designs against and captures its visual baselines in. The
eight preset documents beside them — Catppuccin Mocha and Latte, Nord, Tokyo
Night, Gruvbox Dark, Dracula, and Solarized Dark and Light — carry the same key
set with palettes transcribed from their upstream schemes, are registered by
`ThemeRegistry::new`, and hold no baseline of their own. See `PROVENANCE.md`
P11 for what each one owes upstream and where a value had to move to clear a
gate.

`schema.json` documents their portable JSON contract. `TokenDocument::parse`
rejects incomplete or unknown fields, invalid values, and every required color
pair below its contrast floor, so applications receive the same validation
when registering their own documents at runtime.

Rules:

- surfaces establish hierarchy before borders;
- accent remains a compact action/focus color;
- semantic colors only communicate status;
- `color.loader.*` is the neutral vocabulary of work in progress: the moving
  `mark`, the quiet `track` it travels, the `placeholder` shape of absent
  content, and the `sheen` that crosses it. Waiting is not information, so
  none of the four carries a hue — colour on a loading surface is the
  caller's meaning, never the library's decoration;
- `color.sequence.categorical` is the categorical series scale: exactly eight
  colors in a fixed order, separated by hue rather than by lightness, because
  four slices of a donut are four different things and not four degrees of
  one. A chart takes them by index and cycles;
- `color.node.*` is the node canvas vocabulary. An idle port and a connected
  one are different facts, and so are a resting edge and a live one, so each
  pair is two roles rather than one grey at two alphas. `grid` and
  `gridStrong` are the dot grid and its major interval, `labelWash` is the
  chip an edge label sits on, and `headerWash` is the band behind a node's
  title when the node declares no category colour;
- every elevation step above `flat` casts two layers: a tight ambient contact
  shadow first, then the soft key. A single cast puts a surface at one
  distance from the page in every direction at once, which is what makes a
  card read as a sticker printed on the page rather than as a thing above it;
- typography steps keep size, line-height, and weight together;
- repeated semantic geometry is tokenized;
- one-off component geometry may remain local;
- RGBA tokens use `#RRGGBBAA`.

After editing:

```bash
cargo test -p gpui-box-kit-tokens
cargo run -p xtask -- tokens generate
cargo run -p xtask -- tokens check
```
