use crate::p2p_protocol::{MessageType, ProtocolMessage, MAX_MESSAGE_SIZE, P2P_PROTOCOL_VERSION};

use sha2::{Digest, Sha256};

use std::io::{self, Read, Write};
use std::net::TcpStream;

const MESSAGE_MAGIC: [u8; 4] = *b"NIOM";

const HEADER_SIZE: usize = 4 + 4 + 1 + 4;
const CHECKSUM_SIZE: usize = 32;

const MIN_FRAME_SIZE: usize = HEADER_SIZE + CHECKSUM_SIZE;
const MAX_FRAME_SIZE: usize = HEADER_SIZE + MAX_MESSAGE_SIZE + CHECKSUM_SIZE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageFrame {
    pub version: u32,
    pub message_type: MessageType,
    pub payload: Vec<u8>,
}

impl MessageFrame {
    pub fn new(message_type: MessageType, payload: Vec<u8>) -> Result<Self, String> {
        let message = ProtocolMessage::new(message_type, payload.clone())?;

        message.validate()?;

        Ok(Self {
            version: P2P_PROTOCOL_VERSION,
            message_type,
            payload,
        })
    }

    pub fn to_protocol_message(&self) -> Result<ProtocolMessage, String> {
        if self.version != P2P_PROTOCOL_VERSION {
            return Err("unsupported p2p protocol version".to_string());
        }

        let message = ProtocolMessage::new(self.message_type, self.payload.clone())?;

        message.validate()?;

        Ok(message)
    }
}

pub struct P2pMessageCodec;

impl P2pMessageCodec {
    pub fn write_message(stream: &mut TcpStream, message: &ProtocolMessage) -> io::Result<()> {
        message.validate().map_err(io::Error::other)?;

        let frame = MessageFrame {
            version: message.version,
            message_type: message.message_type,
            payload: message.payload.clone(),
        };

        let encoded = encode_frame(&frame)?;

        stream.write_all(&encoded)?;
        stream.flush()?;

        Ok(())
    }

    pub fn read_message(stream: &mut TcpStream) -> io::Result<ProtocolMessage> {
        let frame = read_frame(stream)?;

        frame.to_protocol_message().map_err(io::Error::other)
    }
}

pub fn encode_frame(frame: &MessageFrame) -> io::Result<Vec<u8>> {
    if frame.version != P2P_PROTOCOL_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unsupported p2p protocol version",
        ));
    }

    if frame.payload.len() > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "message payload exceeds maximum size",
        ));
    }

    let message = ProtocolMessage::new(frame.message_type, frame.payload.clone())
        .map_err(io::Error::other)?;

    message.validate().map_err(io::Error::other)?;

    let payload_length = u32::try_from(frame.payload.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "payload length exceeds u32"))?;

    let total_size = HEADER_SIZE
        .checked_add(frame.payload.len())
        .and_then(|v| v.checked_add(CHECKSUM_SIZE))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "frame size overflow"))?;

    if total_size > MAX_FRAME_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "frame exceeds maximum size",
        ));
    }

    let checksum = checksum(frame.version, frame.message_type, &frame.payload);

    let mut buffer = Vec::with_capacity(total_size);

    buffer.extend_from_slice(&MESSAGE_MAGIC);

    buffer.extend_from_slice(&frame.version.to_be_bytes());

    buffer.push(frame.message_type as u8);

    buffer.extend_from_slice(&payload_length.to_be_bytes());

    buffer.extend_from_slice(&frame.payload);

    buffer.extend_from_slice(&checksum);

    Ok(buffer)
}

pub fn decode_frame(buffer: &[u8]) -> io::Result<MessageFrame> {
    if buffer.len() < MIN_FRAME_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "frame is too small",
        ));
    }

    if buffer.len() > MAX_FRAME_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame exceeds maximum size",
        ));
    }

    let mut cursor = 0usize;

    let magic = read_array_4(buffer, &mut cursor)?;

    if magic != MESSAGE_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid message magic",
        ));
    }

    let version = read_u32(buffer, &mut cursor)?;

    if version != P2P_PROTOCOL_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported p2p protocol version",
        ));
    }

    let message_type_byte = read_u8(buffer, &mut cursor)?;

    let message_type = MessageType::from_u8(message_type_byte)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "unknown message type"))?;

    let payload_length = read_u32(buffer, &mut cursor)? as usize;

    if payload_length > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "payload exceeds maximum size",
        ));
    }

    let expected_size = HEADER_SIZE
        .checked_add(payload_length)
        .and_then(|v| v.checked_add(CHECKSUM_SIZE))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "frame size overflow"))?;

    if buffer.len() != expected_size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame length does not match payload length",
        ));
    }

    let payload_end = cursor
        .checked_add(payload_length)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "payload boundary overflow"))?;

    let payload = buffer[cursor..payload_end].to_vec();

    cursor = payload_end;

    let received_checksum = read_array_32(buffer, &mut cursor)?;

    let expected_checksum = checksum(version, message_type, &payload);

    if received_checksum != expected_checksum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "message checksum mismatch",
        ));
    }

    let message = ProtocolMessage::new(message_type, payload.clone()).map_err(io::Error::other)?;

    message.validate().map_err(io::Error::other)?;

    Ok(MessageFrame {
        version,
        message_type,
        payload,
    })
}

pub fn read_frame(stream: &mut TcpStream) -> io::Result<MessageFrame> {
    let mut header = [0u8; HEADER_SIZE];

    stream.read_exact(&mut header)?;

    let mut cursor = 0usize;

    let magic = read_array_4(&header, &mut cursor)?;

    if magic != MESSAGE_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid message magic",
        ));
    }

    let version = read_u32(&header, &mut cursor)?;

    if version != P2P_PROTOCOL_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported p2p protocol version",
        ));
    }

    let message_type_byte = read_u8(&header, &mut cursor)?;

    let _message_type = MessageType::from_u8(message_type_byte)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "unknown message type"))?;

    let payload_length = read_u32(&header, &mut cursor)? as usize;

    if payload_length > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "payload exceeds maximum size",
        ));
    }

    let body_size = payload_length
        .checked_add(CHECKSUM_SIZE)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "frame size overflow"))?;

    let mut body = vec![0u8; body_size];

    stream.read_exact(&mut body)?;

    let mut full_frame = Vec::with_capacity(HEADER_SIZE + body_size);

    full_frame.extend_from_slice(&header);
    full_frame.extend_from_slice(&body);

    decode_frame(&full_frame)
}

fn checksum(version: u32, message_type: MessageType, payload: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();

    hasher.update(version.to_be_bytes());

    hasher.update([message_type as u8]);

    hasher.update((payload.len() as u32).to_be_bytes());

    hasher.update(payload);

    let digest = hasher.finalize();

    let mut result = [0u8; 32];

    result.copy_from_slice(&digest);

    result
}

fn read_u8(buffer: &[u8], cursor: &mut usize) -> io::Result<u8> {
    if *cursor >= buffer.len() {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "truncated u8"));
    }

    let value = buffer[*cursor];

    *cursor += 1;

    Ok(value)
}

fn read_u32(buffer: &[u8], cursor: &mut usize) -> io::Result<u32> {
    if buffer.len().saturating_sub(*cursor) < 4 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "truncated u32",
        ));
    }

    let bytes = [
        buffer[*cursor],
        buffer[*cursor + 1],
        buffer[*cursor + 2],
        buffer[*cursor + 3],
    ];

    *cursor += 4;

    Ok(u32::from_be_bytes(bytes))
}

fn read_array_4(buffer: &[u8], cursor: &mut usize) -> io::Result<[u8; 4]> {
    if buffer.len().saturating_sub(*cursor) < 4 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "truncated magic",
        ));
    }

    let mut result = [0u8; 4];

    result.copy_from_slice(&buffer[*cursor..*cursor + 4]);

    *cursor += 4;

    Ok(result)
}

fn read_array_32(buffer: &[u8], cursor: &mut usize) -> io::Result<[u8; 32]> {
    if buffer.len().saturating_sub(*cursor) < 32 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "truncated checksum",
        ));
    }

    let mut result = [0u8; 32];

    result.copy_from_slice(&buffer[*cursor..*cursor + 32]);

    *cursor += 32;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p_protocol::MessageType;

    fn ping_frame() -> MessageFrame {
        MessageFrame::new(MessageType::Ping, Vec::new()).expect("ping frame")
    }

    fn transaction_frame() -> MessageFrame {
        MessageFrame::new(MessageType::Transaction, vec![1, 2, 3, 4]).expect("transaction frame")
    }

    #[test]
    fn ping_frame_roundtrip() {
        let frame = ping_frame();

        let encoded = encode_frame(&frame).expect("encode");

        let decoded = decode_frame(&encoded).expect("decode");

        assert_eq!(decoded, frame);
    }

    #[test]
    fn transaction_frame_roundtrip() {
        let frame = transaction_frame();

        let encoded = encode_frame(&frame).expect("encode");

        let decoded = decode_frame(&encoded).expect("decode");

        assert_eq!(decoded, frame);
    }

    #[test]
    fn invalid_magic_is_rejected() {
        let frame = ping_frame();

        let mut encoded = encode_frame(&frame).expect("encode");

        encoded[0] = b'X';

        assert!(decode_frame(&encoded).is_err());
    }

    #[test]
    fn invalid_version_is_rejected() {
        let frame = ping_frame();

        let mut encoded = encode_frame(&frame).expect("encode");

        encoded[4..8].copy_from_slice(&(P2P_PROTOCOL_VERSION + 1).to_be_bytes());

        assert!(decode_frame(&encoded).is_err());
    }

    #[test]
    fn unknown_message_type_is_rejected() {
        let frame = ping_frame();

        let mut encoded = encode_frame(&frame).expect("encode");

        encoded[8] = 255;

        assert!(decode_frame(&encoded).is_err());
    }

    #[test]
    fn oversized_payload_is_rejected() {
        let payload = vec![0u8; MAX_MESSAGE_SIZE + 1];

        assert!(MessageFrame::new(MessageType::Transaction, payload,).is_err());
    }

    #[test]
    fn checksum_tampering_is_rejected() {
        let frame = transaction_frame();

        let mut encoded = encode_frame(&frame).expect("encode");

        let last = encoded.len() - 1;

        encoded[last] ^= 0xFF;

        assert!(decode_frame(&encoded).is_err());
    }

    #[test]
    fn payload_tampering_is_rejected() {
        let frame = transaction_frame();

        let mut encoded = encode_frame(&frame).expect("encode");

        encoded[HEADER_SIZE] ^= 0xFF;

        assert!(decode_frame(&encoded).is_err());
    }

    #[test]
    fn truncated_frame_is_rejected() {
        let frame = ping_frame();

        let encoded = encode_frame(&frame).expect("encode");

        let truncated = &encoded[..encoded.len() - 1];

        assert!(decode_frame(truncated).is_err());
    }

    #[test]
    fn wrong_payload_length_is_rejected() {
        let frame = transaction_frame();

        let mut encoded = encode_frame(&frame).expect("encode");

        let wrong_length = 999u32.to_be_bytes();

        encoded[9..13].copy_from_slice(&wrong_length);

        assert!(decode_frame(&encoded).is_err());
    }

    #[test]
    fn empty_ping_is_valid() {
        let frame = ping_frame();

        assert_eq!(frame.message_type, MessageType::Ping);

        assert!(frame.payload.is_empty());
    }
}
