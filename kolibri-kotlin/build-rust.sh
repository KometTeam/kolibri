#!/usr/bin/env bash
# Builds the native JNI library (libkolibri_kotlin) for the kolibri-kotlin binding.
#
#   ./build-rust.sh            # host build (release) -> rust/target/release/, for the JVM
#   ./build-rust.sh --debug    # host build (debug)
#   ./build-rust.sh --android  # per-ABI .so -> library/src/main/jniLibs/<abi>/ (needs cargo-ndk)
#
# JVM: the release dir is on java.library.path via the Gradle scripts.
# Android: point an androidLibrary's sourceSets jniLibs at library/src/main/jniLibs.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
manifest="$here/rust/Cargo.toml"

android=false
profile=release
for arg in "$@"; do
  case "$arg" in
    --android) android=true ;;
    --debug)   profile=debug ;;
    --release) profile=release ;;
    *) echo "unknown flag: $arg" >&2; exit 2 ;;
  esac
done

if $android; then
  if ! command -v cargo-ndk >/dev/null 2>&1; then
    echo "cargo-ndk not found. Install it with: cargo install cargo-ndk" >&2
    exit 1
  fi
  out="$here/library/src/main/jniLibs"
  flag=()
  [ "$profile" = release ] && flag=(--release)
  # The four ABIs a modern Android app ships. Drop any you don't need.
  cargo ndk \
    -t arm64-v8a -t armeabi-v7a -t x86_64 -t x86 \
    -o "$out" \
    build "${flag[@]}" --manifest-path "$manifest"
  echo "Android .so -> $out"
else
  flag=()
  [ "$profile" = release ] && flag=(--release)
  cargo build "${flag[@]}" --manifest-path "$manifest"
  echo "host lib -> $here/rust/target/$profile/"
fi
