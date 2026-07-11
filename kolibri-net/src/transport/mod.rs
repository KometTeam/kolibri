//! Persistent TLS connection with seq-multiplexed request/response and a
//! broadcast stream of server pushes. tokio + rustls over the protocol codec.

mod client;
mod dispatcher;
mod error;
pub(crate) mod tls;

pub use client::{Client, ClientConfig};
pub use error::TransportError;
