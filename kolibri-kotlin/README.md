# kolibri-kotlin

Kotlin/Android binding for [kolibri-net](../kolibri-net), over **JNI**. A
`Session` owns a tokio runtime in the Rust core; every network call blocks until
it completes. The public API wraps those blocking calls in coroutines: `suspend`
methods run on `Dispatchers.IO`, and server pushes / ws2 notifications arrive as
a `Flow`. A `session.blocking` view exposes the raw synchronous calls for
non-coroutine code.

Payloads cross the boundary either as raw msgpack `ByteArray`, or — with the
`*Json` methods — as JSON strings, so you don't need a msgpack library. Binary
fields in JSON are tagged `{"$bin":"<base64>"}`.

## Layout

| Path | What |
|------|------|
| `rust/` | JNI crate (`kolibri-kotlin-jni`) exposing `Java_ru_kolibri_Native_*` over kolibri-net. |
| `library/` | The Kotlin library: `Session`, `Call`, `Auth`, `Config`. |
| `example/` | A runnable handshake demo. |
| `build-rust.sh` | Builds `libkolibri_kotlin` for the host JVM or per-ABI for Android. |

## Build

The Kotlin library loads a native `libkolibri_kotlin` via `System.loadLibrary`, so
build that first.

**JVM (desktop/server):**

```bash
./build-rust.sh                 # -> rust/target/release/libkolibri_kotlin.{dylib,so}
gradle :example:run --args "api.oneme.ru"
gradle :library:test            # runs the pure-path smoke tests
```

The Gradle scripts put `rust/target/release` on `java.library.path`. Override
with `-Pkolibri.native.dir=/path/to/dir` if you build elsewhere.

**Android:**

```bash
cargo install cargo-ndk        # once
./build-rust.sh --android      # -> library/src/main/jniLibs/<abi>/libkolibri_kotlin.so
```

Then consume the Kotlin sources from an Android library module whose
`sourceSets["main"].jniLibs.srcDir("src/main/jniLibs")` points at that output
(or copy the `.so` files into your app's `jniLibs`). The same `ru.kolibri.*`
sources compile unchanged on Android — only the native packaging differs.

## Use

```kotlin
import ru.kolibri.*

val config = Config(
    host = "api.oneme.ru",
    proxy = "socks5://user:pass@127.0.0.1:1080",   // optional
    onWire = { e -> println("${e.direction} ${e.command} op=${e.opcode} ${e.json}") },
)

Session.open(config).use { session ->
    val info = session.connect()                    // handshake -> JSON string
    session.pingInteractive = false                 // foreground/background hint, live

    // Build requests without a msgpack library — the core does the encoding:
    val js  = session.requestJson(64, """{"field":"value"}""")   // JSON in/out
    val raw = session.request(64, msgpackBytes)                  // or raw bytes

    // Server pushes as a Flow:
    session.pushes().collect { push -> println("op=${push.opcode} ${push.json}") }
}
```

`Session` and `Call` hold native handles — use them in `use { }` or call
`close()`. A finalizer frees the handle as a last resort, but don't rely on it.

### Auth fingerprint

```kotlin
val mode = Auth.fingerprint(callsSeed = seed, deviceId = deviceId)  // 96 bytes
```

Defaults to the reference-client digests; pass `signature`/`dex`/`so` to override.

### Calls (ws2 signaling)

```kotlin
val params = Call.decodeVcp(vcp, conversationId = convId)          // JSON or null
val call = Call.connect(url = ws2Url, userAgent = session.userAgent)
call.use {
    call.accept()
    call.transmitSdp(participantId, type = "offer", sdp = sdp)
    call.notifications().collect { n -> /* raw JSON */ }
}
```

Signaling only — the WebRTC media stack stays in your app. Parse the JSON
responses/notifications with whatever JSON library you use;
`Call.parseConnection` / `Call.parseTransmittedData` pre-shape the ws2
notifications for you.
