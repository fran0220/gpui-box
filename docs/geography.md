# Local geographic visualization

`gpui_kit::display::geography` provides `GeoMap`, immutable validated `GeoData`,
and f64 projection/camera types. The `geography` exhibit uses original synthetic
regions, not boundaries of any real territory. No tiles, geocoding, network,
provider assets, GeoJSON parser, geodesic engine, or 3D renderer is included.

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
| Click map | Select topmost hit, or clear on empty background/hole |
| Click feature readout; Enter/Space on focused readout | Select its stable ID |
| Wheel/trackpad scroll | Pan in projected coordinates |
| Ctrl-wheel | Pointer-anchored zoom |
| Arrow keys on focused map | Pan by 0.1 / zoom unit-world |
| + / − | Zoom at camera center |
| Home | Reset center and zoom |
| [ / ] | Previous/next source identity, with wrap |
| Escape | Clear selection |

The camera uses uniform scale, preserving the projection's aspect ratio. It
fits the unit-world square initially; equirectangular uses the central half of
that square vertically. Zoom is bounded to 1..=64 and camera center to [0,1]².
At center limits, a pointer anchor may move. Wheel pan is supported; drag-pan,
touch gestures, hover tooltips, and automatic fit-to-features are not claimed.

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
current camera. Bounds use the existing prepaint measurement infrastructure,
including device-pixel layout snapping, and settle after a resize measurement.
Hosts must not use secrets or
unredacted sensitive text for labels or IDs.

## Preparation and validation envelope

Prepare `GeoData` once, share it through `Rc`, and reuse it across renders.
Validation is quadratic in ring edges including hole-pair checks. Redraw and
hit testing are linear in all vertices/points; there is no spatial index,
simplification, or large-dataset frame-rate guarantee. The current exhibit is a
small local map, not evidence for national-scale GIS files.

The focused tests use independent projected values, exact cutoff boundaries,
asymmetric screen coordinates, polygon and hole boundaries, winding reversal,
invalid/nested/touching topology, antimeridian splits, source identity/value
refusal, point priority, controlled input and truthful semantic states. Linux
offscreen captures cover both themes and default/zoomed/selected/stale states.
macOS/Windows native input and renderer acceptance remain integration work.

## References and source provenance

The implementation and fixture are original GPUI Box code; no third-party
source was copied or translated. Formula reference:
[PROJ Web Mercator mathematical definition](https://proj.org/en/stable/operations/projections/webmerc.html).
Geometry contract reference:
[RFC 7946 §§3.1.6 and 3.1.9](https://www.rfc-editor.org/rfc/rfc7946).
The projected-edge subset above intentionally differs from full GeoJSON.
Existing GPUI/Lyon path tessellation is reused without framework changes.
