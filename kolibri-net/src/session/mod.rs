//! Session state machine on top of the [`crate::transport`] client: connect →
//! sessionInit handshake → online, with keepalive pings and exponential-backoff
//! reconnect. Device values for the handshake are supplied by the host; the wire
//! shape and the connect/ping/reconnect sequence live here.

mod config;
mod manager;

pub use config::{HandshakeConfig, SessionConfig, UserAgent};
pub use manager::{HandshakeInfo, Session, SessionState};
