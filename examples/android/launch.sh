#!/usr/bin/env bash
# Launch an already-installed development APK on the explicitly selected device.
set -euo pipefail
case "${1:-}" in
    smoke) activity=MainActivity ;;
    reference) activity=ReferenceActivity ;;
    *) echo 'Usage: bash examples/android/launch.sh smoke|reference [am-start options]' >&2; exit 2 ;;
esac
shift
: "${ANDROID_SERIAL:?Set ANDROID_SERIAL to the intended authorized device}"
adb shell am start -W -n "dev.gpui.box.example/.$activity" "$@"
