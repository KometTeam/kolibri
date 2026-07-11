//! Komet binary protocol: packet layout, wire codec, stream framing,
//! compression, and operation codes. This module is transport-agnostic and has
//! no async or I/O dependencies — it turns bytes into [`Packet`]s and back.

pub mod codec;
pub mod compress;
pub mod framing;
pub mod opcodes;
pub mod packet;

pub use codec::{decode, encode, encode_with_cmd, packet_total_len, CodecError, COMPRESSION_THRESHOLD};
pub use framing::{OverflowError, PacketReceiver};
pub use packet::{cmd, Packet, HEADER_SIZE, PROTOCOL_VERSION};
