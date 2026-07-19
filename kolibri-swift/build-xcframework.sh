#!/usr/bin/env bash
# Builds CKolibri.xcframework: the kolibri-swift-ffi static library (a C ABI over
# kolibri-net) packaged with the header + module map so SwiftPM can link it.
#
#   ./build-xcframework.sh            iOS device + iOS simulator + macOS (shipping)
#   ./build-xcframework.sh --macos    macOS only (fast local dev/test; ./build-rust.sh)
set -euo pipefail

macos_only=0
[[ "${1:-}" == "--macos" ]] && macos_only=1

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
lib=libkolibri_swift.a
build="$here/.xcframework-build"
out="$here/CKolibri.xcframework"

rm -rf "$build" "$out"
mkdir -p "$build/macos"

rel() { echo "$here/rust/target/$1/release/$lib"; }
cargo_build() { echo ">> cargo build --target $1"; cargo build --release --manifest-path "$here/rust/Cargo.toml" --target "$1"; }

# macOS slice (universal) — always built.
cargo_build aarch64-apple-darwin
cargo_build x86_64-apple-darwin
lipo -create "$(rel aarch64-apple-darwin)" "$(rel x86_64-apple-darwin)" -output "$build/macos/$lib"

xcargs=(-library "$build/macos/$lib" -headers "$build/headers")

if [[ "$macos_only" -eq 0 ]]; then
  mkdir -p "$build/ios-device" "$build/ios-sim"
  cargo_build aarch64-apple-ios
  cargo_build aarch64-apple-ios-sim
  cargo_build x86_64-apple-ios
  cp "$(rel aarch64-apple-ios)" "$build/ios-device/$lib"
  lipo -create "$(rel aarch64-apple-ios-sim)" "$(rel x86_64-apple-ios)" -output "$build/ios-sim/$lib"
  xcargs=(
    -library "$build/ios-device/$lib" -headers "$build/headers"
    -library "$build/ios-sim/$lib" -headers "$build/headers"
    "${xcargs[@]}"
  )
fi

# Headers bundled into every slice: the real header plus the module map that
# names the CKolibri module the Swift code imports.
mkdir -p "$build/headers"
cp "$here/rust/kolibri.h" "$build/headers/kolibri.h"
cp "$here/module/module.modulemap" "$build/headers/module.modulemap"

xcodebuild -create-xcframework "${xcargs[@]}" -output "$out"

rm -rf "$build"
echo "built: $out $([[ $macos_only -eq 1 ]] && echo '(macOS only)')"
