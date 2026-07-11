use super::compress::{self, CompressError};
use super::packet::{cmd, Packet, HEADER_SIZE, PROTOCOL_VERSION};
use thiserror::Error;

/// Payloads smaller than this are sent uncompressed (matches the Dart client).
pub const COMPRESSION_THRESHOLD: usize = 32;

#[derive(Debug, Error)]
pub enum CodecError {
    #[error("buffer smaller than header ({0} B)")]
    ShortHeader(usize),
    #[error("payload length {declared} exceeds buffer ({actual} B)")]
    PayloadOutOfRange { declared: usize, actual: usize },
    #[error(transparent)]
    Compress(#[from] CompressError),
}

/// Encode a request packet: `payload` is already-serialized MessagePack bytes.
///
/// Payloads of at least [`COMPRESSION_THRESHOLD`] bytes are LZ4-frame
/// compressed; the header flag byte carries `(raw_len / comp_len) + 1` (a size
/// multiplier hint), otherwise 0 for uncompressed — bit-for-bit the Dart
/// `packPacket` layout.
pub fn encode(opcode: u16, payload: &[u8], seq: u16) -> Vec<u8> {
    encode_with_cmd(cmd::REQUEST, opcode, payload, seq)
}

/// Encode a packet with an explicit command byte. The client only ever sends
/// [`cmd::REQUEST`]; this exists for server-side / test construction of ok,
/// error, not-found responses and pushes.
pub fn encode_with_cmd(cmd: u8, opcode: u16, payload: &[u8], seq: u16) -> Vec<u8> {
    let (body, flag): (Vec<u8>, u8) = if payload.len() < COMPRESSION_THRESHOLD {
        (payload.to_vec(), 0)
    } else {
        let compressed = compress::compress_lz4_block(payload);
        let flag = ((payload.len() / compressed.len().max(1)) + 1) as u8;
        (compressed, flag)
    };

    let mut out = Vec::with_capacity(HEADER_SIZE + body.len());
    out.push(PROTOCOL_VERSION);
    out.push(cmd);
    out.extend_from_slice(&seq.to_be_bytes());
    out.extend_from_slice(&opcode.to_be_bytes());
    let packed_len = (((flag as u32) & 0xFF) << 24) | ((body.len() as u32) & 0x00FF_FFFF);
    out.extend_from_slice(&packed_len.to_be_bytes());
    out.extend_from_slice(&body);
    out
}

/// Read the declared total length (header + payload) of the packet whose header
/// begins at `buf[0]`. Returns `None` if fewer than [`HEADER_SIZE`] bytes are
/// available.
pub fn packet_total_len(buf: &[u8]) -> Option<usize> {
    if buf.len() < HEADER_SIZE {
        return None;
    }
    let packed_len = u32::from_be_bytes([buf[6], buf[7], buf[8], buf[9]]);
    let payload_len = (packed_len & 0x00FF_FFFF) as usize;
    Some(HEADER_SIZE + payload_len)
}

/// Decode a single complete packet buffer (header + full payload) into a
/// [`Packet`], decompressing the payload when the header flag is set.
pub fn decode(buf: &[u8]) -> Result<Packet, CodecError> {
    if buf.len() < HEADER_SIZE {
        return Err(CodecError::ShortHeader(buf.len()));
    }

    let ver = buf[0];
    let cmd_byte = buf[1];
    let seq = u16::from_be_bytes([buf[2], buf[3]]);
    let opcode = u16::from_be_bytes([buf[4], buf[5]]);
    let packed_len = u32::from_be_bytes([buf[6], buf[7], buf[8], buf[9]]);
    let comp_flag = (packed_len >> 24) as u8;
    let payload_len = (packed_len & 0x00FF_FFFF) as usize;

    if payload_len == 0 {
        return Ok(Packet {
            ver,
            cmd: cmd_byte,
            seq,
            opcode,
            payload: Vec::new(),
        });
    }

    let end = HEADER_SIZE + payload_len;
    if end > buf.len() {
        return Err(CodecError::PayloadOutOfRange {
            declared: payload_len,
            actual: buf.len(),
        });
    }
    let slice = &buf[HEADER_SIZE..end];

    let payload = if comp_flag != 0 {
        compress::decompress(slice)?
    } else {
        slice.to_vec()
    };

    Ok(Packet {
        ver,
        cmd: cmd_byte,
        seq,
        opcode,
        payload,
    })
}
