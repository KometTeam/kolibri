//! # kolibri-net
//!
//! Reusable core of the Komet messaging protocol: a hand-rolled binary framing
//! over a persistent TLS TCP socket, with MessagePack payloads and LZ4/Zstd
//! compression.
//!
//! Phase 1 (this module set) covers the pure, I/O-free protocol layer:
//! [`protocol::encode`] / [`protocol::decode`], the [`protocol::PacketReceiver`]
//! stream de-framer, and compression. Later phases add the async transport,
//! session state machine, and FFI bindings (Dart via flutter_rust_bridge,
//! Python via PyO3) — all over this same core so every client shares one
//! protocol implementation.

pub mod protocol;

#[cfg(feature = "transport")]
pub mod transport;

#[cfg(feature = "transport")]
pub mod session;

pub use protocol::{
    cmd, decode, encode, opcodes, Packet, PacketReceiver, HEADER_SIZE, PROTOCOL_VERSION,
};

#[cfg(feature = "transport")]
pub use transport::{Client, ClientConfig, TransportError};

#[cfg(feature = "transport")]
pub use session::{HandshakeConfig, HandshakeInfo, Session, SessionConfig, SessionState, UserAgent};
