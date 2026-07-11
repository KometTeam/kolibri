/// Wire header layout (10 bytes, big-endian):
///
/// ```text
/// [0]      ver       protocol version (u8, default 10)
/// [1]      cmd       command type (u8)
/// [2..4]   seq       sequence number (u16 BE)
/// [4..6]   opcode    operation code (u16 BE)
/// [6..10]  packedLen high byte = compression flag, low 24 bits = payload length
/// [10..]   payload   MessagePack, optionally compressed
/// ```
pub const HEADER_SIZE: usize = 10;

pub const PROTOCOL_VERSION: u8 = 10;

/// Command types. Direction decides the meaning of `REQUEST` (client request vs
/// server push both carry cmd == 0).
pub mod cmd {
    pub const REQUEST: u8 = 0;
    pub const PUSH: u8 = 0;
    pub const OK: u8 = 1;
    pub const NOT_FOUND: u8 = 2;
    pub const ERROR: u8 = 3;
}

/// A decoded protocol packet. `payload` holds the decompressed MessagePack
/// bytes (empty when the packet has no body). Decoding into a concrete value is
/// left to the caller via [`Packet::value`] so the core stays representation
/// agnostic (Dart Map, Python dict, Rust struct — all from the same bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub ver: u8,
    pub cmd: u8,
    pub seq: u16,
    pub opcode: u16,
    pub payload: Vec<u8>,
}

impl Packet {
    pub fn is_ok(&self) -> bool {
        self.cmd == cmd::OK
    }

    pub fn is_error(&self) -> bool {
        self.cmd == cmd::ERROR
    }

    pub fn is_not_found(&self) -> bool {
        self.cmd == cmd::NOT_FOUND
    }

    /// A push arrives with cmd == 0 (same as an outgoing request); only
    /// meaningful for incoming packets.
    pub fn is_push(&self) -> bool {
        self.cmd == cmd::PUSH
    }

    /// Decode the payload into a dynamic MessagePack value. Returns
    /// `Ok(Value::Nil)` for an empty payload.
    pub fn value(&self) -> Result<rmpv::Value, rmpv::decode::Error> {
        if self.payload.is_empty() {
            return Ok(rmpv::Value::Nil);
        }
        rmpv::decode::read_value(&mut &self.payload[..])
    }
}
