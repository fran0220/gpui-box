#!/usr/bin/env bash
set -euo pipefail
root=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
case "${1:-}" in
  '') example=ios_gpui; executable=GpuiNative; bundle=dev.gpui-box.ios-native ;;
  --reference) example=ios_reference; executable=GpuiReference; bundle=dev.gpui-box.ios-reference ;;
  *) echo 'Usage: build-gpui-simulator.sh [--reference]' >&2; exit 2 ;;
esac
if [[ $(uname -s) != Darwin ]]; then
  echo 'GPUI iOS compilation requires macOS and full Xcode.' >&2
  exit 2
fi
case "$(uname -m)" in
  arm64) target=aarch64-apple-ios-sim ;;
  x86_64) target=x86_64-apple-ios ;;
  *) echo 'Unsupported simulator host architecture' >&2; exit 2 ;;
esac
export IPHONEOS_DEPLOYMENT_TARGET=16.0
export SDKROOT=$(xcrun --sdk iphonesimulator --show-sdk-path)
rustup target add "$target"
cargo build --manifest-path "$root/Cargo.toml" -p gpui-box-ios --example "$example" --target "$target"
app="$root/target/ios-native/$executable.app"
mkdir -p "$app"
cp "$root/target/$target/debug/examples/$example" "$app/$executable"
cp "$root/examples/ios-native/Info.plist" "$app/Info.plist"
plutil -replace CFBundleIdentifier -string "$bundle" "$app/Info.plist"
plutil -replace CFBundleName -string "$executable" "$app/Info.plist"
plutil -replace CFBundleExecutable -string "$executable" "$app/Info.plist"
plutil -lint "$app/Info.plist"
codesign --force --sign - "$app"
printf 'Built actual GPUI simulator app: %s\nNative execution and visual inspection remain required.\n' "$app"
