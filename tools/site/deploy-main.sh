#!/usr/bin/env bash
# Deploy the exact `origin/main` revision from this checkout, then prove BWG
# serves it.
#
# This is the ordinary deployment path. It runs on the Linux orb (or any
# Linux machine holding the deployment identity) right after `git push`, so the
# hosted catalog follows `origin/main` without a hosted build runner.
#
# Identity resolution, in order:
#
# 1. `GPUI_BOX_DEPLOY_SSH_KEY` names a private key file;
# 2. otherwise `ORIGINGAME_SSH_PRIVATE_KEY_B64` (an Amp environment secret) is
#    written once to `$HOME/.ssh/gpui-box-deploy` with mode 0600.
#
# The receiver is `GPUI_BOX_DEPLOY_HOST`, defaulting to
# `${ORIGINGAME_BWG_SSH_USER:-root}@$ORIGINGAME_BWG_SSH_HOST`, and its host key
# is always the committed `tools/site/ops/bwg-known-hosts`.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

# Capture the activation precondition before validating main. A builder that
# observed the previous deployment cannot overwrite a later successful one.
export GPUI_BOX_EXPECTED_REVISION="$(curl --fail --silent --show-error --connect-timeout 5 --max-time 15 https://gpui-box.origingame.dev/build-info.json | jq -er '.revision')"
git fetch --quiet origin main
head="$(git rev-parse HEAD)"
main="$(git rev-parse origin/main)"
if [[ "$head" != "$main" ]]; then
  echo "HEAD $head is not origin/main $main; push first, then deploy" >&2
  exit 1
fi
if [[ -n "$(git status --porcelain --untracked-files=normal)" ]]; then
  echo "the working tree is dirty; a release must equal $head exactly" >&2
  exit 1
fi

if [[ -z "${GPUI_BOX_DEPLOY_SSH_KEY:-}" ]]; then
  if [[ -z "${ORIGINGAME_SSH_PRIVATE_KEY_B64:-}" ]]; then
    echo "no deployment identity: set GPUI_BOX_DEPLOY_SSH_KEY or ORIGINGAME_SSH_PRIVATE_KEY_B64" >&2
    exit 1
  fi
  install -d -m 0700 "$HOME/.ssh"
  key="$HOME/.ssh/gpui-box-deploy"
  if [[ ! -f "$key" ]]; then
    umask 077
    printf '%s' "$ORIGINGAME_SSH_PRIVATE_KEY_B64" | base64 -d > "$key"
    umask 022
  fi
  export GPUI_BOX_DEPLOY_SSH_KEY="$key"
fi

if [[ -z "${GPUI_BOX_DEPLOY_HOST:-}" ]]; then
  if [[ -z "${ORIGINGAME_BWG_SSH_HOST:-}" ]]; then
    echo "no receiver: set GPUI_BOX_DEPLOY_HOST or ORIGINGAME_BWG_SSH_HOST" >&2
    exit 1
  fi
  export GPUI_BOX_DEPLOY_HOST="${ORIGINGAME_BWG_SSH_USER:-root}@$ORIGINGAME_BWG_SSH_HOST"
fi
export GPUI_BOX_DEPLOY_KNOWN_HOSTS="$root/tools/site/ops/bwg-known-hosts"

tools/site/deploy.sh
tools/site/verify-deployment.sh "$head"
