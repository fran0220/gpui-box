## What changed

<!-- What the library now does, and what it refuses to do. -->

## Checks

- [ ] `cargo run -p xtask -- gate` passes.
- [ ] `cargo run -p xtask -- headless check` passes on macOS and Windows, or the
      change cannot affect rendering.

## Visual regression

CI does not run `xtask scenes check` unless a self-hosted runner is configured,
because the capture needs a composited, frontmost window and a display matching
the one the baselines came from. If this change touches anything that renders,
run it locally and paste the result — the output, not a claim that it passed:

```
cargo run -p xtask -- scenes check
```

- [ ] Not applicable: nothing that renders changed.
- [ ] `scenes check` reported no differences.
- [ ] Images changed, I looked at each one, and they are what this change
      claims. Accepted with `cargo run -p xtask -- scenes capture`.

<details><summary>scenes check output</summary>

```
```

</details>
