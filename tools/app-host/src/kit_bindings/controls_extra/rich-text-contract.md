# RichTextEditor worker boundary

This adapter mounts the native RichTextEditor over its actual
Entity<RichTextEditSession>. It does not convert the rich document into a
TextArea, HTML, or a product model. The document value carries stable block IDs,
text, normalized UTF-8 style runs, links as data, and paragraph alignment/list
metadata. An empty block carries a zero-length style run so its insertion style
survives serialization. Core style/range normalization remains native behavior.

Native keyboard and toolbar edits emit IntentApplied or IntentRefused with the
exact intent. Applied is not a promise that the document changed: the three
native result flags distinguish document, selection, and pending-style changes.
LinkRequested supplies the actual selection; it grants no URL-opening authority.
No Changed event or synthetic lazy snapshot event is invented.

The session getter issues a real, read-only RichTextEditSession reference, not a
snapshot or an Editor reference. Its six queries share the editor's closed
document/selection/style/history result contracts. Lifetime and original getter
identity remain checked by the shared registry. Mutations go through the editor;
there is no mutable session-ref bypass of readOnly or block-ID registration.

Native hard breaks need a synchronous ID factory. The adapter acts as its caller
with a mount-local monotonic allocator and an issued-ID set. Initial documents,
replacement documents and caller intents reserve IDs before mutation; minted IDs
are never reclaimed by undo, deletion or document replacement. The allocator
skips collisions rather than relying on random UUID probability. Core still
rejects duplicate current-document IDs and malformed multiline ID batches.
An intent can reserve an ID without inserting it; query the document for the
actual result. Remounting establishes a new editing session and allocator.

Presentation changes retain entity, session, selection, and history. Default rows
are five; an absent/removed maxRows means fixed current-minimum height. Replacing
the caller document explicitly replaces authority and clears the former history.
Read-only permits native selection but refuses document edits. Disabled controls
refuse commands, while current read-only queries remain available.

This is not a blanket atomicity guarantee for multi-step native input. In
particular, native paste/IME replacement may select a range before a malformed
multiline replacement is refused. The duplicate-ID rejection leaves document
text intact; it does not promise to roll back a preceding selection operation.
