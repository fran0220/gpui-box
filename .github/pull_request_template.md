## What changed

<!-- What the library now does, and what it refuses to do. -->

## Checks

- [ ] `cargo run -p xtask -- gate` passes, or `gate full` passes for a change
      that can affect rendering.
- [ ] `gate full` or a scoped `cargo run -p xtask -- headless check
      <scene...>` passes on the current platform, or the change cannot affect
      rendering. CI checks the complete macOS, Linux, and Windows catalogs.

## Visual regression

The visual gate is headless and deterministic. If this change touches anything
that renders, run the affected scenes locally and paste the result — the
output, not a claim that it passed:

```
cargo run -p xtask -- headless check <scene...>
```

- [ ] Not applicable: nothing that renders changed.
- [ ] `headless check` reported no differences.
- [ ] Images changed, I looked at each one, and they are what this change
      claims. Accepted with `cargo run -p xtask -- headless capture <scene...>`.

<details><summary>headless check output</summary>

```
```

</details>
