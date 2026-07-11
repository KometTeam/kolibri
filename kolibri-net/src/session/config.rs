use std::time::Duration;

use crate::transport::ClientConfig;

/// The `userAgent` sub-map of the sessionInit handshake. The host (Flutter,
/// Python, …) supplies these device values; the wire field names and nesting
/// live in Rust so every client sends an identical handshake shape.
#[derive(Debug, Clone)]
pub struct UserAgent {
    pub device_type: String,
    pub app_version: String,
    pub os_version: String,
    pub timezone: String,
    pub screen: String,
    pub push_device_type: String,
    pub arch: String,
    pub locale: String,
    pub build_number: i64,
    pub device_name: String,
    pub device_locale: String,
}

/// Everything needed to build the `sessionInit` (opcode 6) payload.
#[derive(Debug, Clone)]
pub struct HandshakeConfig {
    pub instance_id: String,
    pub device_id: String,
    pub client_session_id: i64,
    pub user_agent: UserAgent,
}

/// Full session configuration: how to connect, what handshake to send, and the
/// keepalive / reconnect behaviour.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub client: ClientConfig,
    pub handshake: HandshakeConfig,
    /// Keepalive ping interval once the session is online.
    pub ping_interval: Duration,
    /// `interactive` flag sent in the ping payload (false = ghost/offline mode).
    pub ping_interactive: bool,
    /// Reconnect automatically after an unexpected disconnect.
    pub auto_reconnect: bool,
}

impl SessionConfig {
    pub fn new(client: ClientConfig, handshake: HandshakeConfig) -> Self {
        Self {
            client,
            handshake,
            ping_interval: Duration::from_secs(30),
            ping_interactive: true,
            auto_reconnect: true,
        }
    }
}
