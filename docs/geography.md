# Local geographic visualization

`gpui_kit::display::geography` provides `GeoMap`, immutable validated `GeoData`,
and f64 projection/camera types. The `geography` exhibit uses original synthetic
regions, not boundaries of any real territory. No tiles, geocoding, network,
provider assets, geodesic engine, or 3D renderer is included. Local GeoJSON
ingestion supports the explicit subset below.

## Supported geometry is explicit

- Input is longitude/latitude degrees, east/north positive, no altitude.
- Equirectangular accepts ±180° longitude and ±90° latitude. Its unit-world
  formulas are x = longitude / 360 + 0.5 and y = 0.5 − latitude / 360.
- Spherical Web Mercator accepts ±180° longitude and latitude up to
  ±atan(sinh(π)) ≈ ±85.0511287798066°. Its unit-world formulas are
  x = longitude / 360 + 0.5 and y = 0.5 − asinh(tan(latitude radians)) / (2π).
  This is not ellipsoidal Mercator and exposes no distance or area measurement.
- Edges connect projected vertices with straight lines. This is deliberately
  not a claim of full GeoJSON edge interpolation, geodesic interpolation, or
  arbitrary coordinate-system support. Densification belongs to the caller.
- Rings contain at least four positions with an exactly repeated closing
  position. Nonfinite, repeated, degenerate, self-intersecting, or overlapping
  backtracking edges are refused. Straight forward collinear segments are valid.
- Holes are strictly inside the exterior. They cannot cross/touch the exterior,
  overlap/touch one another, or nest. Either winding is accepted; GPUI's existing
  even-odd fill renders real transparent holes. The outer boundary is selectable;
  hole interiors and boundaries are not. Boundary strokes are decorative.
- Any edge spanning more than 180° longitude is refused. Split an antimeridian
  feature into separate polygons ending at +180° and starting at −180°. The map
  does not wrap or duplicate worlds. Exactly 180° edges use the straight projected
  interpretation, not an inferred shortest spherical route.
- Multiple polygons per feature support islands and caller-cut features.
  Overlaps are allowed and paint in source order; last feature wins hit ties.
  Holes remove only their polygon, not underlying independent polygons.
- The normalized predicate tolerance is 1e-12 for cross products and coordinate
  comparisons; nearly touching/tiny geometry may be refused. This is bounded
  floating-point topology, not an exact-arithmetic GIS validator.

`GeoData::new` validates the entire collection before returning it. Nonempty,
unique IDs are shared across features and point overlays. Unsupported input
must become `GeoState::Refused`, never a partly successful map. There is no
silent coordinate clamp or repair; only validated Mercator endpoints receive
floating-point roundoff correction. Applications can use `GeoRefusal::Unsupported`
to report an unsupported source geometry or projection instead of substituting one.

## Values, state, and interaction stay caller-owned

The finite color domain must be strictly increasing, and each observed value
must lie inside it. `None` is unobserved, while `Some(0.0)` is a real reading.
The continuous color interpolation uses the theme's info/accent colors; the
legend samples that same interpolation. Missing values have a separate neutral
swatch. Endpoint labels and feature value formatting are caller-supplied.

Points paint above polygons at a fixed five-logical-pixel radius, independent
of zoom; last point wins ties. Point hit testing uses the same radius and camera.
Polygon hit testing uses the validated projected rings, not their bounding boxes.

`GeoMap::selected` and `GeoMap::viewport` are strictly controlled. `on_event`
emits `GeoEvent::Select` or `GeoEvent::Viewport`; the host applies the proposal
and rerenders, or leaves the previous view unchanged. No handler installs no
interactive handlers. A removed selection target is not silently replaced.

| Input | Proposal |
| --- | --- |
| Click/release map without dragging | Select topmost hit, or clear on empty background/hole |
| Captured left drag / one-contact touch pan | Propose translated camera, including outside the map |
| Pinch | Scale about the moving contact centroid |
| Click feature readout; Enter/Space on focused readout | Select its stable ID |
| Wheel/trackpad scroll | Pan in projected coordinates |
| Ctrl-wheel | Pointer-anchored zoom |
| Arrow keys on focused map | Pan by 0.1 / zoom unit-world |
| + / − | Zoom at camera center |
| Home | Reset center and zoom |
| F | Fit prepared features and points with 16 logical pixels of padding |
| [ / ] | Previous/next source identity, with wrap |
| Escape | Cancel active drag to its initial camera; otherwise clear selection |

The camera uses uniform scale, preserving the projection's aspect ratio. It
fits the unit-world square initially; equirectangular uses the central half of
that square vertically. Zoom is bounded to 1..=64 and camera center to [0,1]².
At center limits, a pointer anchor may move. A window pointer cancellation or
touch cancellation proposes the gesture's original camera, never a selection
completion. The caller can refuse rollback too. Replacing the prepared `Rc`,
removing data or changing measured size interrupts active exploration. Keep the
same immutable `Rc` during a gesture. Pinch promotion starts from the camera
supplied when that pinch begins; it does not replay unaccepted pan proposals.

Hover in interactive maps publishes `<map>.hover-readout` in the existing tooltip
layer. It uses exact caller labels/formatted values for the topmost projected
hit, never interpolated numeric values. The readout follows the pointer and
disappears over holes, outside the map, or during manipulation.

Accepted external camera targets and choropleth color/style targets transition
with Kit's theme motion policy; retargets start from the currently displayed
value. Camera interpolation uses f64 centers and logarithmic zoom. Pointer,
wheel, touch and keyboard manipulation snap to accepted targets. Reduced motion
and `GeoMap::animate(false)` settle immediately. Painting, geometry bounds and
hit testing use the same displayed camera. Geometry changes use keyed opacity
crossfades, never vertex interpolation through possibly invalid topology.
Initial mount intentionally snaps fully visible, including with motion enabled;
enter fades apply to subsequent source additions, not the initial dataset.
Retired shapes are decorative: they lose picking and semantic authority
immediately. Incoming shapes gain visual bounds and picking only at nonzero
opacity; accessible readouts always describe current source values. Interrupted
fades retain sampled opacity; direct manipulation and reduced motion settle.
Retired layers paint behind current geometry; this is not order-preserving
compositing for overlapping features with different colors.

Loading, Empty, Ready, Stale, Unavailable, Error, and Refused are distinct.
Stale retains the caller's last verified data plus the refresh/refusal reason.
The component does not fetch or cache source data. A Ready empty collection
reports Empty. A Stale empty collection still reports Stale and its reason.
`HasPhase` maps Stale to Error with `is_stale()`, and Refused to Unavailable.
The status node's machine value preserves the more specific state name while
its description exposes the shared phase. Built-in status/refusal labels use
the existing string registry, including English and Simplified Chinese packs.
`status_text` replaces the complete status/reason line with caller-owned wording.
Feature/point labels and feature values remain caller text; selection uses visual
highlighting and semantic selected state rather than a hardcoded word.

Semantic IDs are `<map>.status`, `<map>.map`, `<map>.legend`, and
`<map>.feature.<source-id>` for the accessible selectable readout. Separate
`<map>.geometry.<source-id>` image targets expose the visible geographic shape's
projected, viewport-clipped envelope. These are measurement targets, not
rectangular hit regions: canvas selection uses the true polygon geometry.
Entirely offscreen geometry (or a camera entirely inside a hole) has no visual
target; its readout remains available. Multipart features expose one enclosing
rectangle, which can contain empty space, and decorative strokes are excluded.
Semantic nodes expose formatted values, selected state, status/reason, and the
current camera. `MeasuredLeafBatch` computes geometry envelopes in current
prepaint, using current map bounds and the displayed camera. Fractional bounds
retain projected coordinates rather than old per-target Div layout rounding.
Diagnostics keep the map as parent; native leaves sit inside the noninteractive
`<map>.geometry-targets` Group beneath the map. Native physical bounds additionally
follow framework ancestor clipping and transforms. The batch does not paint or
intercept input; painting and exact picking do not depend on diagnostic recording.
Hosts must not use secrets or
unredacted sensitive text for labels or IDs.

## Preparation and validation envelope

Prepare `GeoData` once, share it through `Rc`, and reuse it across renders.
Validation is quadratic in ring edges including hole-pair checks. Preparation
builds an immutable bounding-volume tree over individual polygon parts and
points. Hit queries prune envelopes, then apply exact predicates in source paint
order; painting and measured geometry visit visible source candidates. A large
multipart feature still visits all its parts once it becomes a candidate.
Readouts larger than 32 entries use Kit's virtual List with six mounted rows,
stable source keys, and keyboard navigation to initially unmounted identities.
Key reconciliation still visits all source identities. Visible map geometry is
not clustered; retired fading geometry also incurs linear painting work.

An isolated pre-quad orb test-platform measurement (not GPU submission or native FPS)
measured 100,000 points at zoom 64: mount 47 ms, redraw 13 ms, six readout rows
and 119 paint calls. With all points visible at zoom 1, 1,000 points took 248/138
ms mount/redraw; 10,000 took 4.13/2.29 seconds. Each case explicitly removes its
window and asserts no windows remain before starting the next fixture.
Earlier unisolated loop timings are superseded: dropping Harness alone retained
windows that its frame method refreshed together. These are measured envelopes, not
latency guarantees. Large offscreen datasets benefit from culling, but dense
full-world overlays need caller-side aggregation; interactive 10,000-visible-
point maps are not claimed. Run the mounted-work test to measure your platform.

Point markers now use GPUI's radius-5 rounded quad in a 10-pixel square instead
of rebuilding/tessellating a 24-edge path. Source order, raw readings, circular
picking and visible bounds are unchanged. No non-overlap paint layer is used:
overlapping markers must retain ordered alpha compositing.

The isolated `geography_dense_work_breakdown` benchmark separates 10,000-point
prepared traversal from full component, paint-only canvas, and geometry-target
elements. With the original framework bounds index, measured debug redraws were
2.144 s full / 1.826 s paint-only before quads, versus 1.490 s / 1.097 s after.
Quad release redraws were 118.25 ms full / 53.33 ms paint-only / 31.23 ms targets-
only. Prepared bounds/candidates/readout-key traversal measured 8.83 ms debug and
0.40 ms release. These independently measured cases are not additive timing
partitions. `paint_calls` counts element paint visits, not renderer primitives.
The test platform does not submit a GPU frame; release CPU results are not FPS.

With the balanced framework bounds index and unchanged geography source order,
the same isolated fixture measured:

| Work | Debug | Repository release profile |
| --- | --- | --- |
| Full redraw | 498.53 ms | 51.93 ms |
| Paint-only canvas | 115.52 ms | 7.46 ms |
| Geometry-target elements only | 261.32 ms | 28.87 ms |
| Prepared bounds/candidates/readout keys | 8.64 ms | 0.385 ms |

Both-theme offscreen frame/PNG/semantic-JSON round trips measured 2.111 s dark
and 2.062 s light, versus 3.818/3.851 s with the original index. All 46 playback
frames were byte-identical, including dense maps, selected overlap, interrupted
fades and captured input states; all 10,000 measured targets remain present.
Remaining geometry-target work, not only the virtual readout, is material.
These measurements do not establish an interactive dense-map frame rate.

`geography_semantic_work_breakdown` isolates native accessibility and diagnostic
recording in a 2×2 matrix on the same 10,000 target rectangles. Unlike Harness,
which always arms diagnostics, it opens test windows directly and explicitly
activates each facility. It asserts native Image leaf count and diagnostic target
count independently. Every case removes its window, and the median/range of five
warm redraws excludes tree JSON serialization and GPU work. Geometry elements,
their layout and semantic property construction remain present even with both
outputs disabled; their cost is not attributable to diagnostic recording alone.

Repository-release target-only medians (10,000 leaves, five redraws):

| Native | Diagnostics | Median | Range | Mean cost per target at median |
| --- | --- | --- | --- | --- |
| Off | Off | 22.532 ms | 21.458–26.022 ms | 2.253 µs |
| On | Off | 25.243 ms | 24.951–28.519 ms | 2.524 µs |
| Off | On | 23.972 ms | 23.369–25.711 ms | 2.397 µs |
| On | On | 29.847 ms | 27.290–30.712 ms | 2.985 µs |

That Div-based target setup costs 22.5 ms even with both outputs disabled.
Geography now uses the shared measured-leaf batch to avoid per-target elements
while preserving independent native and diagnostic output, rather than bypassing
the shared boundary or dropping identities. Accessible readout rows remain
ordinary interactive elements, distinct from descriptive map geometry.

After batch integration, five executions of the repository-release dense fixture
measured median full redraw 22.910 ms (22.094–26.174 ms), paint-only 9.855 ms,
ordinary target elements 54.589 ms and batched targets 8.344 ms. These are paired
local TestPlatform cases in the same binary, not comparisons to another orb or
GPU frame-rate guarantees. The full map has 63 element paint visits at both
1,000 and 10,000 visible points, while both native and diagnostic outputs still
publish every source target. The earlier pre-batch release results above remain
historical measurements, not an additive decomposition of this run.

Both-theme offscreen frame/PNG/JSON round trips measured 1.787/1.773 seconds.
All 46 frames match the pre-batch renderer bytes with identical raw input.
20,134 geometry records retain exact ids/properties; removed Div quantization
accounts for bound differences up to 0.250 logical pixels in those captures.
Fractional point and accepted camera tests compare against independent projection
math and native physical bounds, not old rounding. Native lifecycle tests verify
retired/transparent removal and stable identity on reappearance. These checks
do not assert a national-scale GIS or native-display performance envelope.

Run `cargo test -p gpui-box-kit --lib geography_dense_work_breakdown -- --ignored
--nocapture`, then repeat with `--release`. The scale scene's dense-data button
mounts the same 10,000-point input. Playback captures its real renderer output
and asserts every visible source geometry target. Its reported round trip
includes rendering, PNG encoding, semantic JSON and process I/O, not GPU time.

`GeoData::simplified(tolerance)` returns a separate immutable display level.
Tolerance is finite in 0..=0.01 projected unit-world distance. Iterative
Douglas–Peucker reduction retains exact source vertices. Full ring/hole topology
is revalidated; a polygon that would become invalid retains its original display
geometry. Its source features/values/IDs never change. Paint, hit index and
measured envelopes all use the simplified display level. Shared borders of
independent features are not coordinated; no topology-wide GIS generalization
or fixed reduction ratio is claimed. `vertex_count()` reports the actual result.
Callers prepare levels outside render and choose an appropriate tolerance;
automatic screen-space LOD and national-scale GIS frame rates are not claimed.

## GeoJSON and finite-world policy

`GeoData::from_geojson` accepts a JSON string up to 32 MiB containing a Feature
or FeatureCollection. Each feature needs an explicit nonempty string/integer
ID, object or null properties, and a Polygon, MultiPolygon or Point geometry
with exactly two coordinates per position. String/integer ID collisions refuse.
The properties callback receives the original JSON object/null and produces
caller labels, optional choropleth values and formatted values. Point values
are retained by `GeoData::point_reading(id)` and exposed in exact hover and
accessible readouts without changing `GeoPoint` struct literals. They are not
choropleth observations and need not fit its color domain; nonfinite readings
are refused. No properties are guessed from common names.
Null geometry, other geometry types, foreign CRS and unsupported dimensions
refuse the entire collection. Foreign members are ignored; bbox never overrides
actual geometry. The parser reuses existing serde_json and does not fetch URIs.

`with_world` also prepares native caller geometry with `GeoWorldPolicy`:

- `Fixed` preserves the original zero-meridian split policy.
- `Centered(longitude)` shifts the central meridian into [-180°,180°].
- `Auto` places the cut in the largest empty longitude-vertex gap. This joins
  local seam-crossing regions and holes automatically. Remaining cut-crossing
  edges are refused; the policy does not infer spherical interiors or duplicate
  worlds. Exactly hemispheric edges retain their projected-edge interpretation.

Original source degrees remain available through `features()` and `points()`.
Use `project_position` / `unproject_position` with the prepared world for overlays,
not zero-meridian projection methods. Longitudes at the inverse seam normalize
to -180°. Auto can choose a different world after refresh; use Centered to keep
camera coordinates stable. `fit_viewport(size,padding)` fits display geometry
and point radii within the existing camera limits, or returns None for empty
data/unusable frame/padding. A zoom-limit-constrained fit may retain extra space.

The focused tests use independent projected values, exact cutoff boundaries,
asymmetric screen coordinates, polygon and hole boundaries, winding reversal,
invalid/nested/touching topology, antimeridian splits, source identity/value
refusal, point priority, controlled input and truthful semantic states. Linux
offscreen captures cover both themes and default/zoomed/selected/stale states.
macOS/Windows native input and renderer acceptance remain integration work.

For exact intermediate and interrupted renderer evidence, build the checkout
headless tool with its `motion`/`frame` playback protocol, then run:

```bash
python3 scripts/geography-playback.py target/geography-playback
cargo test -p gpui-box-kit --lib geography_preparation_and_query_envelope -- --ignored --nocapture
```

Playback asserts actual geometry and interior pixels in both themes at simulated
times, immediate exact value readout, zero-time retarget continuity and zero-time
reduced-motion settling. It writes ten frames and a sample manifest per theme.
This is offscreen renderer evidence, not native-window timing or hardware touch.
On the Linux orb's debug build, 1k/10k/100k synthetic points prepared in roughly
1.2/17.4/305 ms; 1,000 actual-hit queries took 0.38/0.53/0.49 ms. These are
observations, not wall-clock gate thresholds or whole-map rendering claims.

## References and source provenance

The implementation and fixture are original GPUI Box code; no third-party
source was copied or translated. Formula reference:
[PROJ Web Mercator mathematical definition](https://proj.org/en/stable/operations/projections/webmerc.html).
Geometry contract reference:
[RFC 7946 §§3.1.6 and 3.1.9](https://www.rfc-editor.org/rfc/rfc7946).
The projected-edge subset above intentionally differs from full GeoJSON.
Existing GPUI/Lyon path tessellation is reused without framework changes.
