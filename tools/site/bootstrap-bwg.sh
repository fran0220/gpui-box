#!/usr/bin/env bash
# Install GPUI Box-owned host configuration and one restricted deployment key.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
host="${GPUI_BOX_BOOTSTRAP_HOST:-bwg}"
public_key_path="${1:?usage: tools/site/bootstrap-bwg.sh <deploy-key.pub>}"

grep -Eq '^ssh-ed25519 [A-Za-z0-9+/=]+( .*)?$' "$public_key_path" || {
  echo "the deployment key must be an OpenSSH Ed25519 public key" >&2
  exit 1
}

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
cp "$root/tools/site/ops/bootstrap-host.sh" "$stage/"
cp "$root/tools/site/ops/gpui-box.nginx.conf" "$stage/"
cp "$root/tools/site/ops/gpui-box-mcp.service" "$stage/"
cp "$root/tools/site/ops/install-release.sh" "$stage/"
cp "$root/tools/site/ops/receive-release.sh" "$stage/"
cp "$public_key_path" "$stage/deploy-key.pub"

COPYFILE_DISABLE=1 tar -C "$stage" -cf - . | ssh -o BatchMode=yes "$host" '
  set -e
  incoming=$(mktemp -d /tmp/gpui-box-bootstrap.XXXXXX)
  trap '\''rm -rf "$incoming"'\'' EXIT
  tar -xf - -C "$incoming"
  bash "$incoming/bootstrap-host.sh" "$incoming"
'
