#!/usr/bin/env bash
# Accept the macOS and Windows frames a `Platforms` run reported as changed.
#
# Usage: tools/headless-visual/accept-run.sh <run-id>
#
# A failing headless job in `.github/workflows/platforms.yml` uploads only the
# frames that moved, named exactly as the baseline names them. This downloads
# those artifacts and copies each into the baseline set for its renderer, which
# is the same thing `headless capture` does on that machine. Look at every
# image before committing: a changed image is a claim about what the component
# now looks like.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
run="${1:?usage: accept-run.sh <run-id>}"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

gh run download "$run" --repo fran0220/gpui-box --dir "$scratch" --pattern 'headless-diff-*'

accepted=0
for artifact in "$scratch"/headless-diff-*; do
  [[ -d "$artifact" ]] || continue
  case "$(basename "$artifact")" in
    headless-diff-macOS) renderer=macos ;;
    headless-diff-Windows-*) renderer=windows ;;
    *) echo "unexpected artifact $(basename "$artifact")" >&2; exit 1 ;;
  esac
  target="$root/snapshots/headless/$renderer/scenes"
  while IFS= read -r -d '' frame; do
    cp "$frame" "$target/$(basename "$frame")"
    echo "accepted $renderer/$(basename "$frame")"
    accepted=$((accepted + 1))
  done < <(find "$artifact" -name '*.png' -print0)
done

if ((accepted == 0)); then
  echo "run $run uploaded no changed frames; nothing to accept" >&2
  exit 1
fi
echo "$accepted frame(s) copied; review them with view_media or an image viewer, then commit"
