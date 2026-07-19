# kolibri-swift

Swift binding for [kolibri-net](../kolibri-net), over a C ABI (`./rust`). A
`Session` owns a tokio runtime in the Rust core; each network call blocks until
it completes, and the `async` methods run those blocking calls off the Swift
cooperative pool. Payloads cross the boundary as JSON (no msgpack dependency) or
as raw `Data`.

## Build

The package links `CKolibri.xcframework` — the Rust static lib packaged for Apple
platforms — so build that first. Two scripts, same output at the package root;
pick by whether you need iOS:

**Fast, macOS only** — one universal macOS slice, for the dev/test loop:

```bash
./build-rust.sh          # -> CKolibri.xcframework (macos slice)
swift build
swift test
```

**iOS-capable** — iOS device, iOS simulator, and macOS slices:

```bash
./build-xcframework.sh   # -> CKolibri.xcframework (ios + ios-sim + macos)
swift build
swift test

# build the package for a device / simulator:
xcodebuild -scheme Kolibri -destination 'generic/platform=iOS' build
xcodebuild -scheme Kolibri -destination 'generic/platform=iOS Simulator' build
```

`Package.swift` links the xcframework unconditionally, so re-run the relevant
script after changing the Rust. Run everything from the `kolibri-swift`
directory; on Apple platforms the package also links `CoreFoundation` and
`Security`.

## Use

```swift
import Kolibri

var config = Config(host: "api.oneme.ru")
config.proxy = "socks5://user:pass@127.0.0.1:1080"   // optional
config.onWire = { event in                            // optional traffic log
    print("\(event.direction) \(event.command) op=\(event.opcode) \(event.json)")
}

let session = try Session(config: config)

let info = try await session.connect()                // handshake -> [String: Any]
session.pingInteractive = false                       // foreground/background hint, live

let responseJSON = try await session.requestJSON(opcode, #"{"field":"value"}"#)
let raw = try await session.request(opcode, msgpackData)   // or raw bytes

for await push in session.pushes() {                  // decoded push stream
    print("push op=\(push.opcode): \(push.payload)")
}
```

No msgpack dependency: `connect()`, `requestJSON(_:_:)`, and `pushes()` speak
JSON/`[String: Any]`, and the core does the encoding. A `{"$bin":"<base64>"}`
object in a JSON request marks a binary field; binary in a response comes back as
base64. `connectRaw()` / `request(_:_:)` / `nextPush(timeout:)` give raw msgpack
`Data` if you'd rather decode yourself.

### Blocking API

Every network call has a synchronous form under `session.blocking`, for CLI and
server code that is not inside Swift concurrency:

```swift
let info = try session.blocking.connect()
while let push = try session.blocking.nextPush(timeout: 1.0) { /* ... */ }
```

## Surface

- Session: `Session(config:)`, `connect()` (`[String: Any]`), `requestJSON`,
  `request`, `send`, `nextPush` / `pushes()`, `state`, `pingInteractive`,
  `userAgent`, `disconnect`; `connectRaw` / `request` for raw msgpack; the same
  set synchronously under `session.blocking`.
- Media: `uploadFile`, `uploadPhoto`, `uploadVideo` (through the session's proxy).
- Proxy: `Config.proxy` (HTTP CONNECT or SOCKS5, with auth).
- Logging: `Config.onWire` (both directions), `requestJSON`.
- Auth: `Auth.fingerprint` (96-byte anti-spoof fingerprint; digests default to
  the reference client's, override per build).
- Calls (ws2 signaling): `Call.decodeVCP`, `Call.parseConnection`,
  `Call.parseTransmittedData`, and a `Call` client (`connect`, `accept`,
  `hangup`, `transmitSDP`, `transmitCandidate`, `changeMedia`, `sendCommand`,
  `nextNotification` / `notifications()`, `close`). Signaling only — the WebRTC
  media stack stays in your app.

## Distribution

`build-xcframework.sh` produces `CKolibri.xcframework` with iOS device, iOS
simulator, and macOS slices — the artifact you embed in an app. The Rust is a
`staticlib` only (no `cdylib`), so the lib is linked statically into the app and
there is nothing to ship alongside it. The xcframework is a build product and is
git-ignored; rebuild it from source, or wire `build-xcframework.sh` into a
release job to attach it to a tag.

## License

Dual MIT / Apache-2.0, matching the workspace.
