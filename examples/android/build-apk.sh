#!/usr/bin/env bash
# Development-only APK. Does not install SDK packages or accept licenses.
set -euo pipefail
root=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
: "${ANDROID_HOME:?Set ANDROID_HOME to an existing, licensed Android SDK}"
tools="$ANDROID_HOME/build-tools/35.0.0"
ndk="$ANDROID_HOME/ndk/28.2.13676358"
android_jar="$ANDROID_HOME/platforms/android-35/android.jar"
host=linux-x86_64
[[ $(uname -s) != Darwin ]] || host=darwin-x86_64
llvm="$ndk/toolchains/llvm/prebuilt/$host/bin"
for file in "$tools/aapt2" "$tools/d8" "$tools/zipalign" "$tools/apksigner" "$android_jar" "$llvm/aarch64-linux-android33-clang"; do
    [[ -f "$file" ]] || { echo "Missing pinned build prerequisite: $file" >&2; exit 1; }
done
for tool in javac jar keytool zip cargo; do command -v "$tool" >/dev/null; done
out="$root/target/android-apk"
mkdir -p "$out"
rm -rf "$out/classes" "$out/dex" "$out/package"
mkdir -p "$out/classes" "$out/dex" "$out/package/lib/arm64-v8a"
export CC_aarch64_linux_android="$llvm/aarch64-linux-android33-clang"
export CXX_aarch64_linux_android="$llvm/aarch64-linux-android33-clang++"
export AR_aarch64_linux_android="$llvm/llvm-ar"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CC_aarch64_linux_android"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS="-C link-arg=-Wl,-z,max-page-size=16384"
cargo build --manifest-path "$root/Cargo.toml" --locked -p gpui-box-android-example --target aarch64-linux-android
cp "$root/target/aarch64-linux-android/debug/libgpui_android_demo.so" "$out/package/lib/arm64-v8a/"
"$llvm/llvm-strip" --strip-debug "$out/package/lib/arm64-v8a/libgpui_android_demo.so"
mapfile -t sources < <(find "$root/crates/gpui_android/java" "$root/examples/android/java" -name '*.java' -type f | sort)
javac -encoding UTF-8 -source 17 -target 17 -classpath "$android_jar" -d "$out/classes" "${sources[@]}"
jar --create --file "$out/classes.jar" -C "$out/classes" .
"$tools/d8" --min-api 33 --lib "$android_jar" --output "$out/dex" "$out/classes.jar"
"$tools/aapt2" link -I "$android_jar" --manifest "$root/examples/android/AndroidManifest.xml" -o "$out/unsigned.apk"
cp "$out/dex/classes.dex" "$out/package/"
(cd "$out/package" && zip -q -0 "$out/unsigned.apk" classes.dex lib/arm64-v8a/libgpui_android_demo.so)
"$tools/zipalign" -f -P 16 4 "$out/unsigned.apk" "$out/aligned.apk"
if [[ ! -f "$out/debug.keystore" ]]; then
    keytool -genkeypair -keystore "$out/debug.keystore" -storepass android -keypass android \
        -alias androiddebugkey -dname 'CN=Android Debug,O=GPUI Box,C=US' -keyalg RSA -keysize 2048 -validity 10000
fi
"$tools/apksigner" sign --ks "$out/debug.keystore" --ks-key-alias androiddebugkey --ks-pass pass:android --key-pass pass:android --out "$out/gpui-box-debug.apk" "$out/aligned.apk"
"$tools/apksigner" verify --verbose "$out/gpui-box-debug.apk"
"$tools/zipalign" -c -P 16 4 "$out/gpui-box-debug.apk"
printf '\nDevelopment APK: %s\n' "$out/gpui-box-debug.apk"
