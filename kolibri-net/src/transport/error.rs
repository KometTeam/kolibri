use crate::protocol::codec::CodecError;
use crate::protocol::framing::OverflowError;
use thiserror::Error;

/// Errors surfaced by the async transport and request/response layer.
#[derive(Debug, Error)]
pub enum TransportError {
    /// The server returned an error packet (cmd == error).
    #[error("server error: {message}")]
    Server {
        message: String,
        error_key: Option<String>,
    },

    /// The server rejected the auth/login token (`FAIL_LOGIN_TOKEN`); the
    /// session must be re-established.
    #[error("session expired: {0}")]
    SessionExpired(String),

    /// No response arrived within the request timeout.
    #[error("request timed out")]
    Timeout,

    /// The connection was closed before a response arrived.
    #[error("connection closed")]
    ConnectionClosed,

    #[error("connect timed out")]
    ConnectTimeout,

    #[error("TLS error: {0}")]
    Tls(String),

    #[error("invalid configuration: {0}")]
    Config(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Codec(#[from] CodecError),

    #[error(transparent)]
    Overflow(#[from] OverflowError),
}
