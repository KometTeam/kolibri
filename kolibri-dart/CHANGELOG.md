# Changelog

## 0.1.2

- Expose `setTrustMincifryCa` / `trustMincifryCa`: opt in to the bundled
  Минцифры root (Russian Trusted Root + Sub CA) for hosts that chain to it,
  such as `api2.oneme.ru`. Off by default; covers the session socket, media
  uploads and ws2 call signaling.
- Depend on `kolibri-net` from crates.io instead of a workspace path, so the
  Rust core resolves when the package is consumed from pub.dev.

## 0.1.1

- Upgrade to `freezed` / `freezed_annotation` 3.x.
- Add a canonical `example/example.dart` and an `example/README.md`.
- Rewrite the README for pub.dev with quick-start, requirements and API notes.

## 0.1.0

- Initial release: Dart/Flutter bindings for the Kolibri messaging protocol.
- FFI plugin with a Rust core built via cargokit (requires a Rust toolchain
  on the consumer's machine).
- Platforms: Android, iOS, macOS, Linux, Windows.
