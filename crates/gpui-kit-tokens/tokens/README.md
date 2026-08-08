# Tokens

`studio-dark.json` is the bundled semantic theme. `schema.json` documents its
portable JSON contract.

Rules:

- surfaces establish hierarchy before borders;
- accent remains a compact action/focus color;
- semantic colors only communicate status;
- typography steps keep size, line-height, and weight together;
- repeated semantic geometry is tokenized;
- one-off component geometry may remain local;
- RGBA tokens use `#RRGGBBAA`.

After editing:

```bash
cargo test -p gpui-kit-tokens
cargo run -p xtask -- tokens generate
cargo run -p xtask -- tokens check
```
