# Host-owned plugin platform

`PluginPlatform` depends on the JS runtime, never the other way around and never
from Kit. `node --test tools/plugin-platform/test/*.test.mjs` runs real activation,
crash, dependency and upgrade tests. The runnable example is
`tools/app-host/example-plugin`.

Manifests have schema 1, a lowercase dash-separated ID, a strict `major.minor.patch`
version, relative JS/TS entry, a permission list, **exact-version** dependencies,
and commands/keymaps/panels. Unsupported ranges, fields and permissions are rejected.
Command and panel IDs are local to the plugin and namespaced by the host. Keymaps
refer to declared commands, and conflicts reject activation. Panels name node IDs
the plugin mounts. The stock host only presents declared panel subtrees.

## Installation and lifecycle

Use `gpui-app bundle DIRECTORY FILE` to create an offline UTF-8 text bundle. A
bundle contains manifest, files and SHA-256 over their serialized contents; this
checks corruption, **not publisher authenticity**. The format has no archive
extractor or installation scripts. It rejects absolute paths, dot/parent segments,
backslashes, hidden paths, symlinks, non-files, missing entries, unknown manifest
fields and oversized bundles. It supports at most 256 files and 4 MiB content.
Packages with npm dependencies must bundle them beforehand; `node_modules` is not
silently executed by installation.

Install writes into a staging directory and renames only complete contents into an
immutable version directory. Discovery validates receipts and checks installed
contents. Installation never activates code. Enabling requires a functioning OS
sandbox or an explicit full-code-trust choice. Dependencies must already be active
at their exact declared versions. Disabling a prerequisite also disables dependents.
The registry records versions, not permission consent; no grant survives restart
or version replacement.

An upgrade starts a candidate process first. If import/activation fails or times
out, the old process and registry selection remain. Only a ready candidate replaces
the current version; then the old worker is disposed. Rollback starts the previous
immutable version with fresh permissions. Activation-time capability requests are
denied until activation completes, preventing a trial upgrade from silently mutating
host data. A later crash removes that worker's contributions and dependents; it
does not kill independent peers. Persistent storage is shared across versions, so
plugins must keep backward-compatible data formats: this is code rollback, not a
database snapshot/migration system.

The current store is a single-writer local store, not a remote marketplace. Signed
publishers, remote update discovery, dependency range solving and native installer
integration remain incomplete. The app-host UI exposes enable/disable, installed
version selection and explicit rollback. See the runtime README for security limits.

Mutations serialize within one platform object and acquire an exclusive directory
lock across processes. A crash can leave `.mutation-lock`: operations fail closed
until the operator confirms no host owns the store and removes that lock. Automatic
stale-lock recovery and crash-durable fsync/journaling are not implemented. A candidate
that exits while the registry write is pending restores the old selection; the
previous live process is retained. This is tested with a real candidate process.
