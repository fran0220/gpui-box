# Component selection

Names below are the public types in this checkout. Confirm constructors with
`component` before writing them. Do not use gallery-only helpers
(`action_button`, `section_card`, `settings::page`) — those are not Kit API.

| Need | Use |
|---|---|
| Primary, quiet, or destructive action | `Button` with `primary` / `secondary` or `ghost` / `danger` |
| Icon-only action | `IconButton` — accessible name is required |
| Adjacent related actions | `ButtonGroup` or `SplitButton` |
| Compact state label | `Badge` |
| Dot plus state text | `StatusLine` |
| Inline warning/error/info | `Callout` |
| Grouped content | `Card` + `CardHeader`. `Elevated` for a few cards, `Outlined` for a grid, `Ghost` for structure without claiming a plane |
| Rows inside a card | `ListRow` in a **default (unpadded)** `Card` so the hover wash reaches the edge |
| Settings page rhythm | `SettingsSection` + `SettingsRow`. A managed or inapplicable row renders no control |
| Anchored floating content | `Popover` |
| Centered modal | `Dialog` |
| Edge panel | `Drawer` |
| Glass chrome | `Glass` or `Frost` |
| Search/list keyboard reducer | `popover::{classify_key, step, filter_indices, match_rank}` |
| Known list geometry while loading | `Skeleton` |
| Neutral active work | `PulseLoader` |
| Compact active indicator | `GradientSpinner` |
| Async state without stale value | `Loadable` |
| Refresh while preserving data | `AsyncValue` |
| Filter plus a count | `FilterBar` + `ResultCount`. Unknown, refused, and counted are different facts |
| Exactly one choice in a strip | `SegmentedControl` |
| One or none in a strip | `ToggleGroup` with `ToggleSelection::AtMostOne` |
| Tab strip | `Tabs` — it renders the strip only; the caller renders the panel |
| App chrome | `Sidebar`, `Toolbar`, `StatusBar` |
| Narrow trend | `Sparkline` |
| Cartesian / categorized reading | `LineChart` / `BarChart` — host-formatted labels, no inferred locale |
| Failure the host already holds | `FailurePanel` |
| Empty / unstarted / unavailable / failed | `EmptyState` naming which of those holds |
| Find in text | `SearchField` / `FindReplace` |
| Short tabular surface | `Table` |
| Virtualized administrative grid | `DataGrid` |
| Sensitive secret / code | `PasswordInput` / `OneTimeCodeInput` — one editor, redacted semantics |

A component that is already a card but owns a richer semantic node than a
grouping uses `StyledExt::card_surface`, not a nested `Card`.

Do not create a product-specific component in the Kit until two real consumers
show a stable product-neutral contract.
