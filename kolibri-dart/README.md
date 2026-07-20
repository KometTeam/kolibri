# kolibri

Dart/Flutter bindings for the **Kolibri** messaging protocol, powered by a Rust
core ([`kolibri-net`](https://github.com/KometTeam/kolibri)) via
[`flutter_rust_bridge`](https://github.com/fzyzcjy/flutter_rust_bridge) v2.

Async Rust maps to Dart `Future`s and server pushes to a Dart `Stream`. Payloads
cross the boundary as MessagePack — either as raw bytes (`Uint8List`) or, via the
built-in helpers, as plain Dart `Map`s.

- **Full protocol session** — handshake, request/response, server pushes, ping,
  auto-reconnect.
- **Media upload** as a progress `Stream<UploadEvent>`.
- **Call signaling** over WebSocket, with an anti-spoof device fingerprint.
- **All native platforms** — Android, iOS, macOS, Linux, Windows.

## Requirements

This is an FFI plugin whose native library is compiled from Rust **on the
consumer's machine at app build time** (via [cargokit](https://github.com/irondash/cargokit)).
Anyone building an app that depends on `kolibri` therefore needs a
[Rust toolchain](https://rustup.rs) installed. For Android you also need the
NDK and the relevant `rustup` targets (e.g. `aarch64-linux-android`).

## Install

```yaml
dependencies:
  kolibri: ^0.1.1
```

```bash
flutter pub get
```

## Quick start (Flutter)

```dart
import 'package:kolibri/kolibri.dart';

// On Flutter the bundled native library is found automatically.
await initKolibri();

final session = openSession(host: 'api.oneme.ru'); // override device fields to spoof
final info = await session.connect();               // sessionInit handshake
print(info.callsSeed);

// Request/response with Map payloads (msgpack handled by the core).
final resp = await session.requestMap(64, {'text': 'hello'});
print(resp);

// Server pushes as a stream of (opcode, payload) records.
session.pushesMap().listen((push) {
  final (opcode, payload) = push;
  print('push $opcode: $payload');
});

session.disconnect();
```

Prefer raw bytes? `session.request(opcode: 64, payload: msgpackBytes)` returns a
`Uint8List` — "bytes in, bytes out", matching the core.

## Media upload

```dart
await for (final event in session.uploadFile(url: cdnUrl, data: bytes, filename: 'clip.mp4')) {
  switch (event) {
    case UploadEvent_Progress(:final sent, :final total): print('$sent / $total');
    case UploadEvent_Done(:final status, :final body):    print('done $status');
    case UploadEvent_Error(:final message):               print('error $message');
  }
}
```

## Pure Dart (no Flutter)

Build the native library yourself and pass its path to `initKolibri`:

```bash
cargo build --manifest-path rust/Cargo.toml
dart run example/example.dart   # loads rust/target/debug/libkolibri_dart.dylib
```

```dart
await initKolibri(libraryPath: '/path/to/libkolibri_dart.dylib');
```

See the [`example/`](example) directory for runnable scripts covering the
handshake, uploads, call signaling and the device fingerprint.

## Regenerating the bindings

The Dart bindings and freezed classes are checked in. Regenerate them after
changing the Rust API:

```bash
flutter_rust_bridge_codegen generate         # bindings from rust/src/api
dart run build_runner build                   # freezed classes (UploadEvent)
```

## License

MIT. See [LICENSE](LICENSE).
