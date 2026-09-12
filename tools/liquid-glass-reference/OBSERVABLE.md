# Forward observable comparison

`observable.py` and `glass_reference --observable` are a separate protocol from
the original 960×640 labelled fixture. They consume the native low-amplitude v3
case schema: 640×400 points, 2×, explicit light/dark appearance, label-free Regular
capsules at `[148,100,144,40]` and `[296,239,208,72]`. Backgrounds are encoded-sRGB
cosines sampled at device-pixel centers and rounded ties-to-even. The renderer
validates every supplied code, paints device-pixel stripes, and returns real
headless renderer readbacks, not CPU-simulated glass or native frame copies.

Build and run (Python 3.11+, NumPy and Pillow required). The Linux gate uses
this isolated environment; `.agents/setup` installs it automatically in orbs.
For other POSIX checkouts, initialize it explicitly:

```sh
python3 -m venv target/liquid-glass-python
target/liquid-glass-python/bin/python -m pip install -r tools/liquid-glass-reference/requirements.txt
```

Then use its interpreter for capture, comparison and tests:

```sh
cargo build --locked --manifest-path tools/headless-visual/Cargo.toml --example glass_reference
target/liquid-glass-python/bin/python tools/liquid-glass-reference/observable.py capture NATIVE_RUN NATIVE_SOURCE OUTPUT \
  --executable tools/headless-visual/target/debug/examples/glass_reference \
  --revision SOURCE_ARCHIVE_SHA256 --group light-x-p32-m50-a06
target/liquid-glass-python/bin/python tools/liquid-glass-reference/observable.py compare NATIVE_RUN OUTPUT COMPARISON
target/liquid-glass-python/bin/python -m unittest discover -s tools/liquid-glass-reference
cargo test --locked --manifest-path tools/headless-visual/Cargo.toml --example glass_reference
```

Omitting `--group` captures all groups. Selecting groups always retains all ten
phases and four flat controls. Outputs must be new directories. The native
manifest, hashes, active brackets, source identities, repeat images, and exact
backgrounds are revalidated. The producer must return every requested state,
matching request text and exact PNG/raw RGB bytes. Missing/stale/partial outputs,
background mismatches, or unsettled/repeat mismatches cannot produce a candidate
completion marker. Source, executable, native input and output hashes are retained.
Headless capture uses existing synchronous screenshot readback and waits for two
equal consecutive frames (at most 32); this is not an asynchronous performance test.

No parameters means genuinely unmodified production defaults, including
`protect_text=1`. `--parameters` containing exactly `{"protect_text":0}` instead
declares the **material-only baseline**, with all optical defaults unchanged.
The request retains the exact parameter object; candidate and comparison reports
name this distinction explicitly. Future optical fitting should use material-only
requests, not alter optics to compensate for body-text protection. This command
performs no fitting and installs no defaults.

Every case preserves full capsule crops plus an 8-point margin, signed RGB errors,
and fixed named interior/all-rim/top/bottom/left/right masks. Masks are geometric,
not chosen by error, amplitude, or fit acceptance. Errors are in 8-bit encoded
sRGB codes, not linear light. RGB and grayscale metrics are reported separately.
Per-case records retain appearance, size, axis, period, amplitude, mean and phase.
The eight fit phases alone contribute to `training_objective`; the two held-outs
are reported separately and flat controls never enter either objective. Grouped
summaries do not replace per-case errors. The comparison image is deterministic
(first held-out), with fixed display-only error gain, and is not an equivalence
verdict. `report.json` hashes all raw arrays and its review image.

Linux verification exercises WGPU software fallback against captured native macOS
frames. The same factory supports native Metal, but a Linux run proves neither
Metal output nor unselected operating points. Native acquisition, private optical
interpretations, ray-map inference, baseline acceptance and production tuning are
outside this tool's scope.
