# Bounded material-only observable fitting

This is an additive experiment on the exact parent source archive
`de1837ee42cd66184a684a3113881f9d4e87b147aca2a6700823bda1124806a6`,
using the existing `glass_reference --observable` executable unchanged. It does
not install defaults, acquire native frames, or infer phase-derived coordinates.

```sh
python tools/liquid-glass-reference/observable_fit.py NATIVE_RUN NATIVE_SOURCE OUTPUT \
  --executable tools/headless-visual/target/debug/examples/glass_reference \
  --revision archive-de1837ee42cd66184a684a3113881f9d4e87b147aca2a6700823bda1124806a6 \
  --workers 4
python -m unittest discover -s tools/liquid-glass-reference
```

The plan is saved before rendering. Four fixed candidates are evaluated: the
unchanged material-only baseline (`protect_text=0`) and three profiles with
`regular_wash` 0.25/0.35/0.45, `regular_hairline=0.25`, and
`regular_specular=0.03`. All other optical setters retain the hashed defaults.
This is a bounded discrete search, not exhaustive optimization of the renderer.
The wash/rim search follows the earlier **training** baseline's opposite signed
light/dark brightness errors and visible strong outline; it is not an optical
coordinate fit.

Every candidate covers all 32 operating groups: both appearances, x/y, periods
32/64/128, amplitudes 0.06/0.12, and the native mean controls 0.35/0.65 as well
as mean 0.5, exactly where those combinations exist in the native fixture.
Both capsule sizes are scored. Four flat controls are captured once per pass.
The producer requires complete ten-phase groups, but the scoring function never
opens a held-out native image during training. The initial native integrity audit
does read held-outs for repeat/background checks, without computing their errors.

Selection uses only indices 0–7. The objective is the equally weighted mean of
per-case/size interior RGB RMSE plus 0.25 times the all-rim RGB RMSE, in encoded
8-bit sRGB codes. Each appearance selects independently; ties select the first
candidate. Large capsules do not outweigh small ones through pixel counts.
Missing/duplicate scoring cells abort the run. Selection and training hashes are
persisted before a fresh process renders the selected profiles again. Only then
are indices 8–9 compared. No candidate is added after seeing validation errors.

Earlier baseline work had already compared held-outs in four groups. Their
results remain in the full report but are explicitly excluded from the independent
adoption screen. The other 28 groups provide 56 previously unexamined held-out
cases, each evaluated at both sizes. This is independence from parameter selection,
not a new native capture session or a new physical device.

The predeclared engineering screen requires, separately for both appearances:

- mean held-out interior RMSE ≤3 codes and all-rim RMSE ≤5;
- worst group/size two-phase mean ≤5 interior and ≤8 rim;
- at least 20% improvement over the material-only baseline for both regions;
- no group/size regression above 1 code.

These are conservative project screening limits, not empirically established
perceptual equivalence. Passing would still require platform, visual, semantic,
contrast-protection and integration review. Failing this limited candidate grid
does not prove that every possible setting of the current renderer is inadequate.

Each job retains request text, exact PNG/RGB readbacks, material contract, frame
hashes and command in a compressed archive. The top-level plan records executable,
source and native-input hashes; source/input identity is checked again at the end.
Every background must exactly match the native encoded input, and every repeat
must be identical. All fit/validation error crops and fixed named masks survive
regardless of their errors. A result completion marker is written only after the
frozen selection, complete coverage and end-of-run identity checks pass.

The executable uses the existing synchronous headless readback/settling path.
Linux jobs set `LP_NUM_THREADS=2` and use four independent producer processes;
there is no renderer/shader change. This run provides WGPU software-renderer
evidence, not Metal runtime or performance evidence. It does not cover colors,
interactive content, text protection, motion, scattering regimes beyond this
fixture, or production adoption by itself.

The parent's later local 0.2.0 source adds mask escape, a shared Tracking press
spring and foreground scale 1.014, sampled-variance shadow policy, and persistent
resize capture. This frozen-archive experiment does **not** calibrate those later
changes. Any production adoption requires passing evidence on the intended
production source, not transferring the archive's numerical result by assumption.
