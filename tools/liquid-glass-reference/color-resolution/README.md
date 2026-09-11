# Public SwiftUI Color.orange resolution, 2026-09-11 UTC

Exact program: ResolveOrange.swift; exact stdout: output.txt; OS/Xcode/SDK:
environment.txt. No window/capture/NSApplication is opened. No settings change.
Previous native capture sources, fixtures and evidence are untouched.

Run:
```sh
xcrun swiftc -target arm64-apple-macos26.0 -sdk "$(xcrun --sdk macosx --show-sdk-path)" ResolveOrange.swift -o resolve-orange
./resolve-orange
```

On macOS 27.0 build 26A5416b, Xcode/SDK 26.2, Color.orange resolves to encoded
sRGB #FF8D28 in light and #FF9230 in dark (rounded RGB8). Both have opacity 1.
Explicit Color(.sRGB, red:1, green:149/255, blue:0) resolves to #FF9500 in both.
Thus the named input tint differs by appearance before material compositing.

Apple documents Color.Resolved as storing extended-range Linear sRGB:
https://developer.apple.com/documentation/swiftui/color/resolved
Its linearRed/linearGreen/linearBlue fields are linear sRGB, while red/green/blue
accessors are sRGB (encoded):
https://developer.apple.com/documentation/swiftui/color/resolved/linearred
https://developer.apple.com/documentation/swiftui/color/resolved/red
The actual resolved cgColor returned kCGColorSpaceExtendedSRGB on this OS.
NSColor(cgColor:resolved.cgColor).usingColorSpace(.sRGB) independently confirms
the encoded component values, with small Float/CG conversion rounding differences.

EnvironmentValues is constructed afresh for each case and only colorScheme is
overridden; other values retain defaults. These are this OS/SDK's public named
color resolution results, not a platform-invariant color promise, measurement
inside run-005, or the final composited glass fill. Glass may further transform
its tint, and this experiment neither measures nor infers that transformation.
It establishes a concrete input mismatch; it does not prove compositing math is
otherwise correct. No GPUI changes or calibration are performed.
