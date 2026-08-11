# Contributing

1. Read `AGENTS.md`, `PROVENANCE.md`, and the relevant design document.
2. Add or change semantic tokens before duplicating repeated visual values.
3. Keep components independent of application state and host services.
4. Cover pure reducers and state transitions with unit tests.
5. Register gallery controls in the semantic tree.
6. Update the gallery when a component gains a visual state.
7. While iterating on one component, `cargo run -p xtask -- gate only <scene>`
   answers in about a minute. Run `cargo run -p xtask -- gate`, and `gate full`,
   before committing: `gate only` says nothing about the rest of the workspace.
8. After a UI change, run `cargo run -p xtask -- headless check`, look at every
   image it reports, and accept them with `headless capture` only once you have.
   The offscreen harness runs on macOS, Linux, and Windows; see
   `docs/screenshot-testing.md`, and record the result in the pull request.
9. Add a `CHANGELOG.md` entry for anything a reader of the library would
   notice. Say what it now does and what it refuses to do, not which files
   moved.

A rename is a breaking change when it breaks a consumer, and two of the three
things that break one are invisible to the compiler: a token key, which a
host's own theme document is validated against at runtime, and a semantic id,
which a host's tests assert on. Both are covered by "Versioning and
compatibility" in `README.md`, and both belong under a breaking heading in the
changelog.

Public API changes should document:

- the user-visible capability;
- default, hover, pressed, selected, disabled, and focus behavior;
- keyboard and reduced-motion behavior;
- semantic role and required stable id;
- migration impact.

New derived source or assets are not accepted without an exact source URL,
revision, license, and scope entry in `PROVENANCE.md`.
