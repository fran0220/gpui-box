# GPUI product UI review checklist

## Catalog

- [ ] Signatures and scenes came from this repository's catalog, not crates.io 0.1.1.
- [ ] `search_components` was used before inventing a primitive.

## Architecture

- [ ] Product facts remain in the owning application layer.
- [ ] Views only read view models, emit actions, and hold visual transient state.
- [ ] No credential, database, process, filesystem, or transport access entered a component.
- [ ] Blocking work stays off the window thread.

## State

- [ ] Loading, Empty, Unavailable, Error, and Ready are distinct.
- [ ] Refresh failure preserves the last verified value.
- [ ] Completion follows an authoritative update, not a click.
- [ ] Older async attempts cannot overwrite newer state.
- [ ] Disabled or loading controls install no action handler.

## Design

- [ ] Repeated semantic values come from tokens.
- [ ] Large planes use surface roles; canvas/panel/card/raised stay separable.
- [ ] Accent remains compact.
- [ ] Status colors represent real semantics.
- [ ] Card variant matches density: Elevated, Outlined, or Ghost.
- [ ] `ListRow` sits in an unpadded `Card`.
- [ ] Typography, spacing, radius, and density match existing components.
- [ ] Long content has an explicit boundary.
- [ ] Reduced motion is honored.
- [ ] `Responsive` does not guess an unmeasured width.
- [ ] `Slotted` names are in `SLOTS`.

## Interaction

- [ ] Default, hover, pressed, selected, disabled, and focus are defined.
- [ ] Keyboard access is complete.
- [ ] Popover/dialog dismissal and focus restoration work.
- [ ] Overlays occlude controls beneath them.

## Semantics and tests

- [ ] Every action and assertion target has a stable semantic ID.
- [ ] IDs use business identity, not indexes.
- [ ] Snapshot text contains no secrets.
- [ ] Tests assert behavior rather than source text.
- [ ] Checkout headless/gallery capture was inspected.
- [ ] Fixture evidence is not presented as host-backed proof.

## Provenance

- [ ] Derived source has URL, revision, license, and scope.
- [ ] Third-party notices and asset source files are current.
- [ ] Product/provider trademarks were not added to the generic library.
