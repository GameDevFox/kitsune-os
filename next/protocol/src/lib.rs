//! The packets the host and kernel send each other over serial.
//!
//! A packet on the wire is `00 | COBS(type, payload, crc16) | 00`. COBS removes every
//! zero from what it encodes, so a zero can only be a boundary, and a corrupted packet
//! costs that one packet rather than everything after it. Bytes outside a packet are
//! ordinary characters, so a person can still type at the same line.
//!
//! All fields are little-endian. The CRC is CRC-16/CCITT-FALSE, over the type and payload.
#![cfg_attr(not(test), no_std)]

mod message;

pub use message::{Message, Target, UploadStatus, CHUNK_BYTES};

const CRC_INITIAL: u16 = 0xFFFF;
const CRC_POLYNOMIAL: u16 = 0x1021;
const CRC_BYTES: usize = 2;
const CRC32_INITIAL: u32 = 0xFFFF_FFFF;
const CRC32_POLYNOMIAL: u32 = 0xEDB8_8320;
const BOUNDARY: u8 = 0;
/// A COBS code byte is followed by at most this many bytes with no zero among them.
const COBS_MAX_RUN: usize = 254;
const COBS_FULL_RUN_CODE: u8 = 0xFF;

/// The most a message can hold, including its type.
pub const MAX_MESSAGE_BYTES: usize = message::MAX_BYTES;
/// The most a whole packet can take on the wire, boundaries included: COBS adds a byte for
/// each run, and these messages are shorter than one full run.
pub const MAX_PACKET_BYTES: usize = MAX_MESSAGE_BYTES + CRC_BYTES + 1 + 2;

pub fn crc16(data: &[u8]) -> u16 {
    let mut crc = CRC_INITIAL;
    for byte in data {
        crc ^= u16::from(*byte) << 8;
        for _ in 0..u8::BITS {
            crc = if crc & 0x8000 != 0 { crc << 1 ^ CRC_POLYNOMIAL } else { crc << 1 };
        }
    }
    crc
}

/// CRC-32 (the common one, as `zlib` and `cksum -a crc32b` have it), for whole files.
#[must_use]
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = CRC32_INITIAL;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..u8::BITS {
            crc = if crc & 1 != 0 { crc >> 1 ^ CRC32_POLYNOMIAL } else { crc >> 1 };
        }
    }
    !crc
}

/// Encodes `input` into `output`, returning the length, or `None` if it doesn't fit.
pub fn cobs_encode(input: &[u8], output: &mut [u8]) -> Option<usize> {
    let mut code_at = 0;
    let mut written = 1;
    let mut run = 0;
    *output.get_mut(code_at)? = 0;
    for (index, byte) in input.iter().enumerate() {
        if *byte == 0 {
            output[code_at] = run as u8 + 1;
            code_at = written;
            *output.get_mut(code_at)? = 0;
            written += 1;
            run = 0;
            continue;
        }
        *output.get_mut(written)? = *byte;
        written += 1;
        run += 1;
        if run == COBS_MAX_RUN {
            output[code_at] = COBS_FULL_RUN_CODE;
            run = 0;
            if index + 1 < input.len() {
                code_at = written;
                *output.get_mut(code_at)? = 0;
                written += 1;
            } else {
                return Some(written);
            }
        }
    }
    output[code_at] = run as u8 + 1;
    Some(written)
}

/// Decodes `input` into `output`, returning the length, or `None` if it isn't valid.
pub fn cobs_decode(input: &[u8], output: &mut [u8]) -> Option<usize> {
    let mut read = 0;
    let mut written = 0;
    while read < input.len() {
        let code = input[read];
        if code == 0 {
            return None;
        }
        read += 1;
        for _ in 1..code {
            let byte = *input.get(read)?;
            if byte == 0 {
                return None;
            }
            *output.get_mut(written)? = byte;
            written += 1;
            read += 1;
        }
        if code != COBS_FULL_RUN_CODE && read < input.len() {
            *output.get_mut(written)? = 0;
            written += 1;
        }
    }
    Some(written)
}

impl Message {
    /// Writes the whole packet to `output`, returning its length.
    pub fn encode(self, output: &mut [u8; MAX_PACKET_BYTES]) -> usize {
        let mut plain = [0; MAX_MESSAGE_BYTES + CRC_BYTES];
        let mut message = [0; MAX_MESSAGE_BYTES];
        let length = self.to_bytes(&mut message);
        plain[..length].copy_from_slice(&message[..length]);
        let crc = crc16(&message[..length]);
        plain[length..length + CRC_BYTES].copy_from_slice(&crc.to_le_bytes());

        output[0] = BOUNDARY;
        let end = output.len() - 1;
        let encoded = cobs_encode(&plain[..length + CRC_BYTES], &mut output[1..end]).expect("MAX_PACKET_BYTES has room for any message");
        output[encoded + 1] = BOUNDARY;
        encoded + 2
    }
}

/// Decodes what was between two boundaries, for a host reading the kernel's output, where
/// anything that fails is console text.
#[must_use]
pub fn decode_packet(encoded: &[u8]) -> Option<Message> {
    let mut plain = [0; MAX_MESSAGE_BYTES + CRC_BYTES];
    let length = cobs_decode(encoded, &mut plain)?;
    let message_length = length.checked_sub(CRC_BYTES)?;
    let (message, crc) = plain[..length].split_at(message_length);
    if crc16(message).to_le_bytes() != *crc {
        return None;
    }
    Message::from_bytes(message)
}

/// What a byte from the line turned out to be.
#[derive(Debug, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum Received {
    /// An ordinary character, outside any packet.
    Character(u8),
    /// Part of a packet, or a packet too damaged to use.
    Absorbed,
    Message(Message),
}

/// Pulls packets out of a stream of bytes, one byte at a time.
pub struct Receiver {
    encoded: [u8; MAX_PACKET_BYTES],
    length: usize,
    in_packet: bool,
}

impl Receiver {
    #[must_use]
    pub const fn new() -> Self {
        Self { encoded: [0; MAX_PACKET_BYTES], length: 0, in_packet: false }
    }

    /// Whether a packet has started and not finished, which a stray zero can also cause,
    /// so the caller should `abandon` it if the rest doesn't arrive soon.
    #[must_use]
    pub const fn in_packet(&self) -> bool {
        self.in_packet
    }

    pub fn abandon(&mut self) {
        self.in_packet = false;
        self.length = 0;
    }

    pub fn push(&mut self, byte: u8) -> Received {
        if !self.in_packet {
            if byte == BOUNDARY {
                self.in_packet = true;
                self.length = 0;
                return Received::Absorbed;
            }
            return Received::Character(byte);
        }
        if byte != BOUNDARY {
            if self.length < self.encoded.len() {
                self.encoded[self.length] = byte;
                self.length += 1;
            } else {
                self.abandon();
            }
            return Received::Absorbed;
        }
        // Two boundaries in a row: one packet's end was lost, so this is the next one's start.
        if self.length == 0 {
            return Received::Absorbed;
        }
        let decoded = self.decode();
        self.abandon();
        decoded.map_or(Received::Absorbed, Received::Message)
    }

    fn decode(&self) -> Option<Message> {
        decode_packet(&self.encoded[..self.length])
    }
}

impl Default for Receiver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
