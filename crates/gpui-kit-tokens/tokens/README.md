# Tokens

`studio-dark.json` and `studio-light.json` are the bundled semantic themes.
`schema.json` documents their portable JSON contract. `TokenDocument::parse`
rejects incomplete or unknown fields, invalid values, and every required color
pair below its contrast floor, so applications receive the same validation
when registering their own documents at runtime.

Rules:

- surfaces establish hierarchy before borders;
- accent remains a compact action/focus color;
- semantic colors only communicate status;
- `color.loader.gradient` is the working signature, not an accent: it
  appears only while work is in flight and stays off idle chrome;
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
