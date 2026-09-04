# Upstream resynchronization assessment

This is a cost assessment, not an import plan or an update receipt. GPUI Box
remains the development authority, no upstream tree was merged, and the frozen
vendor refs were not advanced.

## Assessed coordinates

The assessment was generated on 2026-09-04 with:

```sh
scripts/sync-zed/sync-zed assess \
  --revision 801c087af22dd189dc1aa49e2f370b4f04190b19
```

| Coordinate | Revision |
| --- | --- |
| Frozen filtered base (`vendor/zed-gpui`) | `c036e5bcb472b7c557c231a66d69e646285d1942` |
| GPUI Box `ours` | `c801eecb34e0448f8a23b7a3d8394a76b48b322c` |
| Zed source snapshot | `801c087af22dd189dc1aa49e2f370b4f04190b19` |

The Zed source is 483 commits ahead of the official frozen baseline
`a6a23c7b80a5cefa0487b7856335be89ace7e483`. The frozen path filter produced
327 files. All 18 configured package manifests were present and all 18 package
names were rewritten to their GPUI Box identities before merging. The JSON
report's SHA-256 is
`30b85f08c93bb864581bed8465e47297764d822393045c5238fd8f1fd5b3a1fa`.

The command fetched and merged only inside a temporary repository. A hash of
the current repository's complete `show-ref` output was identical before and
after the run.

## Measured merge surface

| Measure | Files | Additions | Deletions | Changed/payload lines |
| --- | ---: | ---: | ---: | ---: |
| Filtered upstream change against the frozen base | 152 | 17,548 | 11,358 | 28,906 |
| Automatically merged, fully conflict-free files against `ours` | 93 | 9,029 | 2,373 | 11,402 |
| Conflicts | 57 | — | — | 8,287 |

There are 196 conflict hunks. Five unmarked modify/delete or binary-style
conflicts count as one hunk and zero payload lines each. For text conflicts,
“payload lines” counts both sides between Git's conflict markers and excludes
the marker lines. The 11,402 automatic-merge lines count only staged files with
no conflict anywhere; automatically merged portions of the 57 conflicted files
are deliberately excluded. These measures therefore describe review surface
and must not be added together as a patch-size total.

### Conflicts by subsystem

Classification uses this precedence: `platform_view`, accessibility,
platform renderer, scene, window, text, elements, then other/Kit-unrelated.
The frozen filter contains framework and support crates, not `gpui-kit`, so the
last category means that a conflict has no direct Kit source ownership.

| Subsystem | Files | Hunks | Payload lines |
| --- | ---: | ---: | ---: |
| Window | 5 | 48 | 1,383 |
| Scene | 1 | 20 | 1,558 |
| macOS renderer | 3 | 3 | 0 |
| Windows renderer | 1 | 4 | 559 |
| WGPU renderer | 5 | 33 | 2,242 |
| Web renderer | 0 | 0 | 0 |
| Linux renderer | 0 | 0 | 0 |
| Other renderer | 1 | 1 | 19 |
| Elements | 3 | 20 | 919 |
| Text | 1 | 1 | 7 |
| PlatformView | 0 | 0 | 0 |
| Accessibility | 1 | 1 | 79 |
| Other / Kit-unrelated | 36 | 65 | 1,521 |

### Ten largest conflict files

| File | Subsystem | Hunks | Payload lines |
| --- | --- | ---: | ---: |
| `crates/gpui_wgpu/src/wgpu_renderer.rs` | WGPU renderer | 25 | 1,594 |
| `crates/gpui/src/scene.rs` | Scene | 20 | 1,558 |
| `crates/gpui/src/profiler.rs` | Other / Kit-unrelated | 6 | 988 |
| `crates/gpui/src/window.rs` | Window | 36 | 891 |
| `crates/gpui_windows/src/directx_renderer.rs` | Windows renderer | 4 | 559 |
| `crates/gpui/src/elements/div.rs` | Elements | 4 | 535 |
| `crates/gpui_macos/src/window.rs` | Window | 8 | 444 |
| `crates/gpui_wgpu/src/gpui_wgpu.rs` | WGPU renderer | 1 | 422 |
| `crates/gpui/src/elements/animation.rs` | Elements | 15 | 252 |
| `crates/gpui_wgpu/src/shaders_storage.wgsl` | WGPU renderer | 3 | 158 |

## Already adapted versus absent

This inventory is based on local provenance, local commit history, and the
present code, not patch-id ancestry: the existing ports intentionally use GPUI
Box types, names, tests, and behavior boundaries.

| Zed source revision | Local status | Evidence and boundary |
| --- | --- | --- |
| `8b1497dbd22fb06f5838a7c0b84a1e54fafa71bc` | Adapted | Local `1a37ce5d2017e48acae854563275d762e22db36d` added the framework spring solver, animation lifecycle, and Kit delegation. Zed's product examples were not imported. |
| `956a49e4ca8aa4b7c2c293e1414c91f009824ae3` | Adapted | Local `4bf127119f4ef7a65a4cb15d99d7fc2f250af999` added raw touch and portable recognition at the framework boundary. |
| `76b1096cbd83b5b5138793e5f552218abc8fdcbb` | Adapted | The same local gesture port carries platform-selectable exponential and Android spline scroll physics. |
| `0855410ccd2040efbbf14d71409166b6c472e0bd` | Adapted | The same local gesture port carries long-press recognition and claiming. |
| `b3326e13c142fc8f313aca67a93dd6855a1e7e32` | Adapted | The same local gesture port carries phased `TouchDragEvent` dispatch. |
| `5e28272c1407ced4bae4a90deaea25352a1fbc96` | Adapted | The same local gesture port locks a pan to its initial dominant axis. |
| `a21007b7a948e46afbe719150f5e9968bfcd1078` | Partially adapted | Local `2431dc84dc2ae25ae02b809057fd2a196576d09c` retained GPUI Box's profiler authority and added the selected draw/submission hooks; it did not import Zed's profiler UI and product wiring. |
| `9e236090b9a31338caf233d440f724922b58d7e1` | Partially adapted | Per-window frame timing informed GPUI Box's bounded `FrameTimingMonitor`, but upstream histogram/product dependencies were not imported. |
| `1861e58f984c76afc06032e753557994ffc8fe44` | Deliberately absent | Foreground journals, hang incidents, telemetry, and their product integration were explicitly excluded. |
| `55007f518bc1d49e6b3291c5eaa1aabf649b36fd` | Semantically narrowed | GPUI Box reports dirty-to-**submission**, because return from `PlatformWindow::draw` does not prove compositor/display presentation. It does not claim or import dirty-to-present behavior. |

The four additional fixes called out for this review have mixed status:

| Zed revision | Local status | Evidence |
| --- | --- | --- |
| `eb354c8d504071bdb79110a7a5c9d374c2864113` | Absent | The current Wayland path still has `renderer_presented` and `completed_frame`; it does not have Zed's `schedule_frame`/`FrameLoop` demand-driven state machine. |
| `ae99a867d7a24682435bd1821c66b4e172a10768` | Present in the frozen bootstrap | Both the frozen vendor tree and current X11 path coalesce `Expose` windows, ignore unmapped windows, and immediately request `require_presentation`. This was not a post-freeze manual port. |
| `511ac170363776319b38cc0e9c047a06aa2e7541` | Absent | Inactive-window throttling remains hard-coded to 33,333 µs; `WindowOptions` has no `inactive_frame_interval`. |
| `a85cf449ec6e72274ee6f9e8462f702299b04cce` | Adapted | Despite being a touch-prediction fix rather than a render-loop fix, its `last_movement`/opposing-vector guard, non-reversing momentum, and regression test are present in local gesture port `4bf127119f4ef7a65a4cb15d99d7fc2f250af999`. P18 records the reviewed target revision but does not list this follow-up separately in `source_revisions`; this assessment does not mutate that frozen source receipt. |

This is a scoped inventory of the cited ports and fixes, not a claim that the
remaining commits in the 483-commit range were reviewed individually.

## Cost comparison and decision

Continuing selective ports keeps each framework change product-neutral,
reviewable, attributable to exact source commits, and testable on the affected
platforms. It also avoids automatically adopting product assumptions and
upstream workspace dependencies. Its recurring costs are source archaeology,
translation into diverged APIs, separate provenance, and the risk of missing a
companion fix. The mixed render-loop inventory above demonstrates that risk.

A discrete whole-tree import would preserve more upstream relationships in one
operation, but it is not mechanical: 57 conflicted files contain 196 hunks and
8,287 payload lines, concentrated in the fork's most changed renderer, scene,
window, profiler, and element authorities. The 93 cleanly merged files still
add 11,402 lines of semantic review surface. All affected Linux, macOS,
Windows, Web, accessibility, renderer, generated-API, dependency, and headless
contracts would need validation after conflict resolution; pixel-changing
renderer decisions would additionally require native baseline review.

The measured near-term choice is therefore to continue deliberate manual
ports. A discrete overall import should be treated as a separately staffed
migration and should begin only after explicit policy and provenance sign-off,
not by applying this assessment's temporary merge result.
