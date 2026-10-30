# Changelog

## 0.1.7

- `CallSignaling.notifications()` no longer misses what the server sends right
  after the handshake. The reader routed notifications into a broadcast channel
  before anyone subscribed, so the `connection` notification that starts every
  call was often dropped and the call stayed silent until something poked the
  server. The first subscriber now gets everything since the handshake.
- The notification stream no longer ends when the Dart side falls behind; it
  skips what it missed and keeps going.

## 0.1.6

- Connects on networks with broken, filtering or hijacked DNS. Every connect
  (session socket, media uploads, call signaling) now resolves through the
  system resolver with a timeout of its own, ignores sinkhole answers
  (`0.0.0.0`, `::`, loopback for a public name), falls back to the addresses
  that worked last, and as a last resort asks DNS-over-HTTPS resolvers dialed
  by IP (Yandex, Google, Cloudflare).
- A host's addresses are raced (Happy Eyeballs, RFC 8305) instead of tried one
  after another, with the TLS handshake inside each attempt: one address that
  drops packets or presents a foreign certificate no longer fails the connect.
- New `setDnsCachePath(path: …)` keeps the addresses that worked in a file, so
  they survive restarts. Call it once at startup, before the first connect;
  without it they are kept in memory only.
- `connect_timeout` now covers resolution, TCP and the TLS handshake together;
  the handshake used to get a second timeout of its own.

## 0.1.5

- Disposing a `KolibriSession` or `CallSignaling` no longer blocks the calling
  thread. Their tokio runtime used to wait for every `spawn_blocking` task on
  drop, including a `getaddrinfo` stuck on a network that swallows DNS, so
  releasing the last handle from the UI isolate froze the app for the whole
  resolver timeout. The runtime now shuts down in the background.

## 0.1.4

- Handshake fields that are not set are now omitted from `sessionInit` instead
  of being sent empty: `arch` (empty string), `buildNumber` (0), `instanceId`
  (empty string) and `clientSessionId` (0). A web-type session can therefore
  produce a payload byte-identical to the MAX web client, which sends none of
  them. Mobile sessions pass real values and are unaffected.

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
