# Native editor boundary

TextArea and Editor retain the actual native entity across presentation changes.
Editor.text_area returns that entity through the shared reference registry;
TextArea.focus_handle continues the same ancestor validity and command guards.
Snapshots are values containing the complete document text and revision, not
capability references or metadata standing in for a document.

TextArea `change` and Editor `changed` are legacy string subscriptions. The
adapter looks up a live matching handler before materializing EditSnapshot text.
`edited` emits the native revision, replaced UTF-8 byte range, and inserted text.
Explicit snapshot queries and subscribed language requests materialize complete
text because their closed contracts ask for the document.

Editor language services carry native request/revision identity. Results retain
Idle, Loading, Empty, Ready, Refreshing, Error and Unavailable; retained values
may accompany failures. Native code rejects stale responses and invalid edits.
Diagnostics and semantic tokens are revision paired, with native UTF-8 boundary
and duplicate/overlap validation. No transport, parser or language provider is
installed by enabling language services.

Not exposed by this host:

- External pasted paths and images: no approved read/image capability bridge.
  `pasteRefused` reports Unavailable and a reason, never paths, bytes or fake refs.
- Native indentation callbacks: no serialized synchronous closure capability.
- Parser installation: the app-host Kit dependency does not enable `syntax`.
- Arbitrary native HighlightStyle values: use revision-paired semantic tokens
  and diagnostics; no unvalidated native style object crosses the worker wire.

The exhibit under `fixture/editors` exercises Editor → TextArea → FocusHandle
through real worker queries and an actual native click, and compares both
complete snapshots after editing through the child.
