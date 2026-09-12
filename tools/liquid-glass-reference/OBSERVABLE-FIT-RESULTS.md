# Frozen archive observable fit: screening failed

This completed bounded experiment applies only to parent source archive
`de1837ee42cd66184a684a3113881f9d4e87b147aca2a6700823bda1124806a6`.
It does not calibrate the subsequent production 0.2.0 changes. No defaults were
installed. See `OBSERVABLE-FIT.md` for the frozen plan and protocol.

## Execution and selection

The run completed with exit 0: 160/160 group jobs, 4,860 retained actual WGPU
readback frames, and zero pending renderer jobs. Observed process start was
2026-09-12 02:06:51 UTC; result completion was 05:08:33.629321 UTC: approximately
3 h 1 min 42.63 s including audits and settling, not a GPU performance benchmark.

```sh
python tools/liquid-glass-reference/observable_fit.py NATIVE_RUN NATIVE_SOURCE OUTPUT \
  --executable tools/headless-visual/target/debug/examples/glass_reference \
  --revision archive-de1837ee42cd66184a684a3113881f9d4e87b147aca2a6700823bda1124806a6 \
  --workers 4
```

All 32 native operating groups and both capsule sizes were covered, preserving
appearance, axis, period, mean, amplitude and phase as separate report fields.
The four-candidate search selected light trial 2 and dark trial 1 using only
phases 0–7. Both have `protect_text=0`, `regular_hairline=0.25`, and
`regular_specular=0.03`; light has `regular_wash=0.35`, dark `regular_wash=0.25`.
Every other setter remains at its hashed default. These are **material-only**
candidates, not unmodified production defaults (which protect text).

Selection was persisted before fresh validation renders, and was not changed
after inspecting phases 8–9. The four groups with previously exposed held-outs
remain reported but are excluded from the independent screen: the latter uses
28 groups, 56 held-out cases and both sizes. No phase coordinate map was fitted.

## Independent held-outs fail the predeclared absolute limits

Values are equally weighted per-case/size RGB RMSE in encoded 8-bit sRGB codes,
not pooled pixels or linear-light error. Baseline here means the unchanged
**material-only** baseline; the earlier forward comparison separately retains
the genuinely unmodified production-default baseline.

| Appearance | Region | Material-only baseline | Selected mean | Worst group/size mean |
| --- | --- | ---: | ---: | ---: |
| light | interior | 23.594693 | 4.173667 | 9.265388 |
| light | all rim, 0–3 pt | 34.672549 | 18.299680 | 24.033428 |
| dark | interior | 37.704839 | 11.571231 | 18.468893 |
| dark | all rim, 0–3 pt | 37.412384 | 21.167144 | 29.836727 |

Both appearances fail: mean limits are 3 interior / 5 rim and worst group/size
limits are 5 / 8. Improvement of at least 20% and no regression above 1 code pass,
but do not override the failed absolute limits. All raw errors and named masks,
including rejected regions, remain in the evidence.

Training-only matched-support diagnostics at period 64 and amplitude 0.06 show
signed RGB mean biases across input means 0.35 / 0.50 / 0.65 of
−8.398612 / −0.486235 / +7.676775 for light and
−17.333883 / −12.164924 / −1.846440 for dark. A wash/rim-only search does not
resolve this tone-response dependence. Further testing should include independent
gain/lift or tone-response controls and rim/blur behavior on the intended
production source. This limited grid does not prove that a shader change is
necessary or that all existing parameter settings fail. Future tuning using
these now-observed held-outs must disclose their reuse and establish a new screen.

## Verification and evidence identity

- Python discovery: 67 tests, OK, one existing skip.
- Independent read-only audit: 160 job archives, 4,860 frames, 16,896 metric rows
  and 2,816 error arrays verified against actual native/GPUI pixels.
- Exact backgrounds, repeats, opaque alpha, PNG/raw hashes, request binding,
  independently constructed capsule masks, metrics, selection and screening pass
  the audit. Native/source identity is checked at both ends of the formal run.
- Representative light and dark held-out comparisons were visually inspected;
  substantial rim residuals and dark interior error remain.

The evidence archive retains `result.json`, plan, selection, all training and
validation reports, per-case error arrays/masks, compressed original readbacks,
requests and commands. Its accompanying transfer manifest gives archive and
ordered-part sizes/hashes. Audit script/log, test log and training diagnostics
are packaged alongside the untouched run output.

| Input | SHA256 |
| --- | --- |
| Native source archive | `a743692d4790682d768476e3cfb20bd97f1f99f7e6639943ea47802e5223739d` |
| Native evidence archive | `75e7678c1c6036b6ac576fab2521eaf713feec83b69824ea0a45b458489b659f` |
| Producer executable | `934de3fd68bd9c45fe3bf446f990d594f6e00a4f18cc6678bd0e80432b4057ce` |
| Frozen plan | `59bcb1e61ecc51a97bf4bb2afd0e7d3f4ca3c7c1e4f6eb580dae01343c14637b` |
| Persisted selection | `ee8981995f343856241de2fc359ad08ec31a2c8d1abe364ef235eb91bdd1732e` |

This is software WGPU evidence using existing synchronous headless readback and
settling. It is not Metal runtime verification, perceptual equivalence, a new
native session, or coverage of colored content, text protection, motion or the
later production changes. Screening is complete and negative; production adoption
remains unsupported.
