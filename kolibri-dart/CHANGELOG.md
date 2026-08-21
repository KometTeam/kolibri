# Changelog

## 0.1.3

- `SessionOptions` gains `isPwa` and `headerUserAgent`. Both are optional: when
  left unset the key is absent from the handshake `userAgent` map entirely, so
  existing sessions send exactly the same payload as before.
- These are the two fields the MAX web client sends that mobile clients did not.
  The server routes web push (`sw-web-push`, opcode 22 subscriptions) only to
  sessions that look like an installed web app, so a client wanting web push
  must send `deviceType: WEB`, `pushDeviceType: WEBPUSH` and `isPwa: true`.

## 0.1.2

- Expose `setTrustMincifryCa` / `trustMincifryCa`: opt in to the bundled
  Минцифры root (Russian Trusted Root + Sub CA) for hosts that chain to it,
  such as `api2.oneme.ru`. Off by default; covers the session socket, media
  uploads and ws2 call signaling.
- Depend on `kolibri-net` by git tag instead of a workspace path, so the Rust
  core resolves when the package is consumed from pub.dev. Building an app that
  uses this package now needs git access to the repository.

## 0.1.1

- Upgrade to `freezed` / `freezed_annotation` 3.x.
- Add a canonical `example/example.dart` and an `example/README.md`.
- Rewrite the README for pub.dev with quick-start, requirements and API notes.

## 0.1.0

- Initial release: Dart/Flutter bindings for the Kolibri messaging protocol.
- FFI plugin with a Rust core built via cargokit (requires a Rust toolchain
  on the consumer's machine).
- Platforms: Android, iOS, macOS, Linux, Windows.
