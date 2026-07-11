//! Async transport: a persistent TLS TCP connection with request/response
//! multiplexing over sequence numbers and a broadcast stream of server pushes.
//! Built on tokio + rustls; sits directly on top of the pure [`crate::protocol`]
//! codec.

mod client;
mod dispatcher;
mod error;
mod tls;

pub use client::{Client, ClientConfig};
pub use error::TransportError;
