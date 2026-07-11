use super::codec;
use thiserror::Error;

/// Buffer overflow guard (16 MB) — mirrors the Dart `PacketReceiver`.
pub const MAX_BUFFER_SIZE: usize = 16 * 1024 * 1024;

#[derive(Debug, Error)]
#[error("PacketReceiver buffer overflow ({0} B)")]
pub struct OverflowError(pub usize);

/// Reassembles the raw TLS byte stream into complete framed packets. Feed it
/// arbitrary chunks; it returns whichever whole packets are now available and
/// keeps any partial remainder buffered for the next call.
#[derive(Default)]
pub struct PacketReceiver {
    buf: Vec<u8>,
}

impl PacketReceiver {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Append `data` and drain every complete packet now buffered. Each returned
    /// `Vec<u8>` is a full header+payload slice ready for [`codec::decode`].
    pub fn feed(&mut self, data: &[u8]) -> Result<Vec<Vec<u8>>, OverflowError> {
        self.buf.extend_from_slice(data);

        if self.buf.len() > MAX_BUFFER_SIZE {
            let overflow = self.buf.len();
            self.reset();
            return Err(OverflowError(overflow));
        }

        let mut packets = Vec::new();
        let mut consumed = 0usize;

        loop {
            let remaining = &self.buf[consumed..];
            let Some(total) = codec::packet_total_len(remaining) else {
                break;
            };
            if remaining.len() < total {
                break;
            }
            packets.push(remaining[..total].to_vec());
            consumed += total;
        }

        if consumed > 0 {
            self.buf.drain(..consumed);
        }
        Ok(packets)
    }

    pub fn reset(&mut self) {
        self.buf.clear();
    }

    pub fn buffered_len(&self) -> usize {
        self.buf.len()
    }
}
