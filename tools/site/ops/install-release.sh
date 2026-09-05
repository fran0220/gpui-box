#!/usr/bin/env bash
# Validate, atomically activate, health-check, and retain a GPUI Box release.
set -euo pipefail

[[ "$(id -u)" -eq 0 ]] || { echo "installer must run as root" >&2; exit 1; }
source_dir="${1:?installer needs an extracted bundle directory}"
base="${GPUI_BOX_INSTALL_BASE:-/opt/gpui-box}"
releases="$base/releases"
current="$base/current"
previous="$base/previous"

exec 9>"${GPUI_BOX_INSTALL_LOCK:-/run/lock/gpui-box-release.lock}"
flock -w 30 9
trap 'exit 124' TERM INT

bundle_name="$(basename "$source_dir")"
[[ "$bundle_name" =~ ^gpui-box-linux-x64-([0-9a-f]{12})$ ]] || {
  echo "invalid bundle name: $bundle_name" >&2
  exit 1
}
short_revision="${BASH_REMATCH[1]}"
revision="$(tr -d '\r\n' < "$source_dir/REVISION")"
[[ "$revision" =~ ^[0-9a-f]{40}$ && "${revision:0:12}" == "$short_revision" ]] || {
  echo "REVISION does not match the bundle name" >&2
  exit 1
}

required=(
  REVISION
  EXPECTED_REVISION
  SHA256SUMS
  bin/gpui-box-mcp
  public/index.html
  public/mcp/index.html
  public/mcp/tools.json
  public/api-index.json
  public/developer-index.json
  public/build-info.json
)
for path in "${required[@]}"; do
  [[ -f "$source_dir/$path" ]] || { echo "bundle is missing $path" >&2; exit 1; }
done
(
  cd "$source_dir"
  sha256sum --strict --check SHA256SUMS
)
expected="$(tr -d '\r\n' < "$source_dir/EXPECTED_REVISION")"
[[ "$expected" =~ ^[0-9a-f]{40}$ || "$expected" == none ]] || {
  echo "invalid expected revision" >&2; exit 1;
}
jq -e --arg revision "$revision" '
  .schema == 1 and
  .revision == $revision and
  .catalogSchema == 1 and
  .counts.tools == 10 and
  .counts.components > 0 and
  .counts.symbols > 0 and
  .counts.scenes > 0
' "$source_dir/public/build-info.json" >/dev/null
file "$source_dir/bin/gpui-box-mcp" | grep -Eq 'ELF 64-bit.*x86-64' || {
  echo "MCP executable is not Linux/x86-64" >&2
  exit 1
}
if readelf -l "$source_dir/bin/gpui-box-mcp" | grep -q 'Requesting program interpreter'; then
  echo "MCP executable must be statically linked" >&2
  exit 1
fi

mkdir -p "$releases"
final="$releases/$revision"
if [[ ! -d "$final" ]]; then
  staging="$releases/.staging-$revision-$$"
  trap 'rm -rf "${staging:-}"' EXIT
  mv "$source_dir" "$staging"
  chown -R root:root "$staging"
  find "$staging" -type d -exec chmod 0755 {} +
  find "$staging" -type f -exec chmod 0644 {} +
  chmod 0755 "$staging/bin/gpui-box-mcp"
  runuser -u gpui-box -- "$staging/bin/gpui-box-mcp" check \
    --catalog "$staging/public" --revision "$revision"
  mv "$staging" "$final"
  trap - EXIT
else
  runuser -u gpui-box -- "$final/bin/gpui-box-mcp" check \
    --catalog "$final/public" --revision "$revision"
fi

old_target=""
if [[ -L "$current" ]]; then
  old_target="$(readlink -f "$current")"
fi
installed=none
if [[ -n "$old_target" && -f "$old_target/REVISION" ]]; then
  installed="$(tr -d '\r\n' < "$old_target/REVISION")"
fi
if [[ "$installed" != "$expected" && "$installed" != "$revision" ]]; then
  echo "stale activation: expected $expected, installed $installed" >&2
  exit 1
fi
already_current=false
if [[ "$old_target" == "$final" ]]; then
  already_current=true
  echo "GPUI Box $revision is already current; rechecking services"
else
  if [[ -n "$old_target" && -d "$old_target" ]]; then
    ln -s "$old_target" "$base/.previous-$$"
    mv -Tf "$base/.previous-$$" "$previous"
  fi
  ln -s "$final" "$base/.current-$$"
  mv -Tf "$base/.current-$$" "$current"
fi

rollback() {
  status=$?
  trap - EXIT
  if [[ $status -ne 0 && "$already_current" == false ]]; then
    echo "deployment failed; rolling back" >&2
    if [[ -n "$old_target" && -d "$old_target" ]]; then
      ln -s "$old_target" "$base/.rollback-$$"
      mv -Tf "$base/.rollback-$$" "$current"
      timeout 30 systemctl restart gpui-box-mcp || true
    else
      rm -f "$current"
      timeout 30 systemctl stop gpui-box-mcp || true
    fi
  fi
  exit "$status"
}
trap rollback EXIT

timeout 30 systemctl restart gpui-box-mcp
health=""
deadline=$((SECONDS + 60))
for _ in $(seq 1 30); do
  (( SECONDS < deadline )) || break
  if health="$(curl --connect-timeout 1 --max-time 2 -fsS -H 'Host: gpui-box.origingame.dev' http://127.0.0.1:9350/healthz 2>/dev/null)" \
    && jq -e --arg revision "$revision" \
      '.status == "ok" and .revision == $revision and .toolCount == 10' \
      <<<"$health" >/dev/null; then
    break
  fi
  health=""
  sleep 1
done
[[ -n "$health" ]] || { echo "MCP health check did not reach $revision" >&2; exit 1; }
timeout 15 nginx -t
timeout 30 systemctl reload nginx

current_target="$(readlink -f "$current")"
previous_target="$(readlink -f "$previous" 2>/dev/null || true)"
kept=0
while IFS= read -r release; do
  [[ -n "$release" ]] || continue
  if [[ "$release" == "$current_target" || "$release" == "$previous_target" || $kept -lt 5 ]]; then
    kept=$((kept + 1))
    continue
  fi
  name="$(basename "$release")"
  [[ "$name" =~ ^[0-9a-f]{40}$ ]] && rm -rf "$release"
done < <(find "$releases" -mindepth 1 -maxdepth 1 -type d -name '[0-9a-f]*' -printf '%T@ %p\n' \
  | sort -rn | cut -d' ' -f2-)

trap - EXIT
echo "deployed GPUI Box $revision"
