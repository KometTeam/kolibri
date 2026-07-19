#!/usr/bin/env bash
# Fast local build: a macOS-only CKolibri.xcframework for the dev/test loop.
# For an iOS-capable build, run ./build-xcframework.sh instead.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$here/build-xcframework.sh" --macos
