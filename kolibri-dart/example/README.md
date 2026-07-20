# kolibri examples

Runnable, self-contained scripts. Build the native library once, then point
`KOLIBRI_LIB` at it:

```bash
cargo build --manifest-path rust/Cargo.toml
export KOLIBRI_LIB="$PWD/rust/target/debug/libkolibri_dart.dylib" # .so / .dll elsewhere
```

| File | What it shows |
| --- | --- |
| [`example.dart`](example.dart) | Open a session, run the handshake, listen for pushes |
| [`handshake.dart`](handshake.dart) | The bare `connect()` / `disconnect()` lifecycle |
| [`upload.dart`](upload.dart) | Media upload as a `Stream<UploadEvent>` (progress → done/error) |
| [`call.dart`](call.dart) | Call signaling over a local WebSocket (no network needed) |
| [`fingerprint.dart`](fingerprint.dart) | Build the 96-byte anti-spoof `authMode` fingerprint |

Run any of them with:

```bash
dart run example/example.dart
```

`call.dart` and `upload.dart` spin up a local server and need no network access;
`example.dart` and `handshake.dart` connect to a real host.
