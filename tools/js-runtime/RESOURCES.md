# Native resource boundary

The worker passes a closed `{key}` reference, never a native path, URI, decoder,
asset-source extension, native pointer, or owner token. The native host accepts
registration only after the runtime independently authorizes the `resources`
capability for an active generation. Clipboard, filesystem-read and network
permission do not imply resource permission, nor the reverse.

## Wire formats and limits

`validateResourceRegistration` accepts exactly `{key,mime,data}`. Keys use 1–128
ASCII letters, digits, `_` or `-`. Data is canonical base64, at most 128 KiB
encoded and 96 KiB decoded. A generation owns at most 32 immutable registrations
and 1 MiB retained resource payload. Rejected registrations consume no quota.

Supported MIME types:

- `image/x.gpui-rgba8`: little-endian u32 width and height, then exactly
  width × height × 4 tightly packed RGBA8 bytes. Each dimension is 1–1024 and
  the entire representation must fit the 96 KiB cap. `rgbaResource` constructs
  this representation. The host uses GPUI `RenderImage::from_rgba`, without
  invoking an encoded-image decoder.
- `application/octet-stream`: bounded opaque bytes. Native consumers must use a
  revocable `ResourceBytes` lease and enforce their own parsing/work limits.
  This does not authorize embedded URIs or prove model/animation validity.

PNG, JPEG, GIF, WebP, SVG, compressed archives and other MIME types are refused.
The framework's ordinary compressed-image decoders are not a hard CPU/allocation
sandbox; this boundary does not route untrusted data through them. Registration
performs bounded synchronous work with no concurrent native decode jobs. Audio
and video playback are not implemented by registering bytes.

## Host integration

`Resources::install(cx)` installs a weak App-global view and returns the
host-owned `ResourceStore`. Reinstallation revokes the previous view.

On every tree/grant/lifetime change call
`store.reconcile(&HashMap<mount_token, generation_principal>, cx)` with only
resource-authorized active mounts. Tokens come from the runtime's existing
ownership authority, never descriptors. `register(principal, registration)`
stores a payload once per generation. Same-generation mounts share that payload
and quota. Dropping a mount invalidates its leases without invalidating peers.

`Resources::image(&ResourceRef, &App)` returns `Result<ImageSource>` using the
current mount's `EffectOwner`. Its custom loader checks that mount alias and
registration incarnation on every use. `Resources::bytes` similarly returns a
lease whose `with_bytes(cx, closure)` checks current ownership. Removing then
readding a token/key does not revive an old lease.

Permission removal calls `revoke(principal,cx)` or reconciles without that
principal. Host release must call `clear(cx)` from its release observer while
App is available to evict all image atlases. ResourceStore Drop also clears the
CPU state; factories hold only weak references. Atlas eviction includes a
deferred pass because the currently updating window is temporarily absent from
`App.windows`.

Registration promises must preserve Pending, Ready, Refused and Error rather
than reporting success before the native acknowledgement. Denied/corrupt data
can be corrected and retried because failure does not reserve a key. Successful
keys are immutable. Adapters must show an explicit semantic refusal/error both
for initial resolution errors and for errors from a subsequently revoked image
loader; they must not substitute fixture data or unrestricted path loaders.

## Immutable packaged assets

The manifest `assets` field is a closed array of `{key,path,mime}` entries,
validated by `validateAssetDeclarations`. The trusted
`loadPackagedResources(root,assets)` loader returns frozen validated
registrations, not resource authorization. The runtime must separately authorize
and register them for an active principal.

Every declaration and aggregate budget is checked before reading file contents.
Only bounded regular files are accepted; traversal, unknown MIME, duplicate keys,
symlinks (including interior symlinks) and hardlinks are rejected. Reads are serial,
with one load per package root and at most four concurrently active package loads.
Opening/reading checks inode, size and timestamps, and Linux checks the actual
opened `/proc/self/fd` path before reading bytes. Files are snapshotted, not retained
as paths or open handles.

The caller must supply an approved immutable package tree, not a concurrently
mutable adversary directory. Portable Node has no directory-relative `openat`
primitive; native macOS/Windows adversarial mutation-race protection has not been
executed or claimed. Normal files, traversal/symlink denial and native image
rendering have been exercised on Linux. A writable source tree is not an approved
immutable package merely because its path passes containment validation.

## Focused verification

```sh
node --test tools/js-runtime/test/resources.test.mjs tools/js-runtime/test/resource-package.test.mjs
node tools/app-host/node_modules/typescript/bin/tsc --strict --noEmit --target ES2022 --module NodeNext --moduleResolution NodeNext tools/js-runtime/test/resources-types.ts
cargo test -p gpui-box-app-host --all-features resources
cargo clippy -p gpui-box-app-host --all-targets --all-features -- -D warnings
```

Set `GPUI_RESOURCE_ARTIFACTS` to capture the native approved and revoked images.
The native test checks exact red/blue pixel samples, semantic Ready/Refused state,
same-generation ImageId sharing, revoked-mount errors and released CPU image
ownership. GPUI's existing atlas eviction API is used; the wgpu atlas does not
implement the optional `contains` test instrumentation, so these tests do not
claim a measured GPU allocation count.
