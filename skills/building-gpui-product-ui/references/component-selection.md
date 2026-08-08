# Component selection

| Need | Use |
|---|---|
| Primary, quiet, or destructive action | `button::action_button` |
| Compact state label | `badge::badge` |
| Dot plus state text | `status::status_line` |
| Inline warning/error/info | `status::callout` |
| Grouped settings/list content | `card::section_card` + `card_row` |
| Settings page rhythm | `settings::{page,page_header,subtitle,section_title}` |
| Anchored floating content | `popover::anchored_below` / `anchored_above` |
| Centered modal | `popover::modal` + `dialog_card` |
| Search/list keyboard reducer | `popover::{classify_key,step,filter_indices}` |
| Known list geometry while loading | `loaders::skeleton_rows` |
| Neutral active work | `loaders::pulse_loader` |
| Compact active indicator | `loaders::gradient_spinner` |
| Async state without stale value | `state::Loadable` |
| Refresh while preserving data | `state::AsyncValue` |

Do not create a product-specific component in this repository until two real
consumers show a stable product-neutral contract.
