#!/usr/bin/env bash
# Build and stream one immutable release to GPUI Box's restricted BWG receiver.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
host="${GPUI_BOX_DEPLOY_HOST:-bwg}"
key="${GPUI_BOX_DEPLOY_SSH_KEY:-}"
known_hosts="${GPUI_BOX_DEPLOY_KNOWN_HOSTS:-}"
out="${GPUI_BOX_RELEASE_OUT:-$root/target/site-release}"

archive="$("$root/tools/site/build-release.sh" "$out" | tail -n 1)"
[[ -f "$archive" ]] || { echo "release archive was not built: $archive" >&2; exit 1; }

ssh_args=(
  -o BatchMode=yes
  -o ConnectTimeout=15
  -o ServerAliveInterval=15
  -o ServerAliveCountMax=4
)
if [[ -n "$key" ]]; then
  ssh_args+=(-i "$key" -o IdentitiesOnly=yes)
fi
if [[ -n "$known_hosts" ]]; then
  # Pin the receiver's host key to the committed file instead of whatever the
  # machine's own known_hosts has accumulated.
  ssh_args+=(-o "UserKnownHostsFile=$known_hosts" -o StrictHostKeyChecking=yes)
fi

echo "deploying $(basename "$archive") to $host"
ssh "${ssh_args[@]}" "$host" /usr/local/sbin/receive-gpui-box-release < "$archive"
