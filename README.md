# kolibri

A fast, reusable client core for a binary messaging protocol — hand-rolled
framing over a persistent TLS TCP socket, MessagePack payloads, LZ4/Zstd
compression, and a full session state machine (handshake, keepalive, reconnect).
Written in Rust so the same protocol implementation can be embedded in many
hosts.

Originally extracted from a Flutter messaging client's networking layer, the
core is UI- and platform-agnostic: bytes in, bytes out.

## Layout

| Crate | What |
|-------|------|
| [`kolibri-net`](kolibri-net) | Rust core: packet codec, framing, compression, async TLS transport, session state machine, CDN media upload. |
| [`kolibri-py`](kolibri-py) | Python bindings (pyo3/maturin) — a synchronous `Session` exposing native dicts. |
| [`kolibri-dart`](kolibri-dart) | Dart / Flutter bindings via `flutter_rust_bridge` — async `Future`s + push `Stream`. |

The design goal: write the protocol **once** in Rust, then wrap it with thin
per-language bindings. The session machine (handshake, ping, reconnect) lives in
the core, so every binding gets it for free.

## Quick start

**Rust:**
```rust
use kolibri_net::{Session, SessionConfig, ClientConfig, HandshakeConfig};
let session = Session::new(SessionConfig::new(ClientConfig::new("host", 443), handshake));
let info = session.connect().await?;
let resp = session.request(opcode, &msgpack_bytes).await?;
```

**Python:**
```python
import kolibri
s = kolibri.Session("host", 443)
info = s.connect()
resp = s.request(opcode, {"key": "value"})   # dict in → dict out
```

## Build & test

```bash
cargo test                 # runs the kolibri-net test suite (protocol + transport + session)
cargo clippy

cd kolibri-py              # Python bindings
python -m venv ../.venv && source ../.venv/bin/activate && pip install maturin
maturin develop           # (prefix with PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 on very new Python)
python examples/handshake.py
```

## Status

Protocol core, async TLS transport, and session state machine are done and
verified end-to-end against a live server (full auth + login flow). Python and
Dart bindings both work and drive the same core (handshake verified against the
live server from all three languages).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
