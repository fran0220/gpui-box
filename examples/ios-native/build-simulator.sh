#!/usr/bin/env bash
set -euo pipefail
root=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
if [[ $(uname -s) != Darwin ]]; then
  echo 'Native iOS build requires macOS with full Xcode and an iOS Simulator runtime.' >&2
  exit 2
fi
sdk=$(xcrun --sdk iphonesimulator --show-sdk-path)
arch=$(uname -m)
case "$arch" in arm64|x86_64) ;; *) echo "Unsupported simulator host architecture: $arch" >&2; exit 2;; esac
output="$root/target/ios-native"
app="$output/GpuiHostFixture.app"
mkdir -p "$app"
xcrun --sdk iphonesimulator clang -target "$arch-apple-ios16.0-simulator" \
  -isysroot "$sdk" -fobjc-arc -fmodules -Wall -Wextra \
  -Werror=return-type -Werror=protocol \
  -I"$root/crates/gpui_ios/native" \
  "$root/crates/gpui_ios/native/host.m" "$root/examples/ios-native/fixture.m" \
  -framework UIKit -framework Foundation -framework QuartzCore -framework Metal \
  -o "$app/GpuiHostFixture"
cp "$root/examples/ios-native/Info.plist" "$app/Info.plist"
plutil -lint "$app/Info.plist"
codesign --force --sign - "$app"
printf 'Built simulator-only native host fixture: %s\n' "$app"
printf 'This is NOT GPUI full-platform acceptance. See examples/ios-native/README.md.\n'
