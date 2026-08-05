# Contributing

1. Read `AGENTS.md`, `PROVENANCE.md`, and the relevant design document.
2. Add or change semantic tokens before duplicating repeated visual values.
3. Keep components independent of application state and host services.
4. Cover pure reducers and state transitions with unit tests.
5. Register gallery controls in the semantic tree.
6. Update the gallery when a component gains a visual state.
7. Run the full validation suite from `README.md`.
8. Capture the gallery at 920×900 after UI changes.

Public API changes should document:

- the user-visible capability;
- default, hover, pressed, selected, disabled, and focus behavior;
- keyboard and reduced-motion behavior;
- semantic role and required stable id;
- migration impact.

New derived source or assets are not accepted without an exact source URL,
revision, license, and scope entry in `PROVENANCE.md`.
