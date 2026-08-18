use crate::p2p::PeerId;
use crate::p2p_message::{
    HeaderRef, P2pMessage, P2pMessageType, MAX_BLOCKS_PER_MESSAGE, MAX_HEADERS_PER_MESSAGE,
    MAX_P2P_MESSAGE_SIZE, MAX_REJECT_REASON_LENGTH,
};

use std::io::{self, Read, Write};

const FRAME_LENGTH_SIZE: usize = 4;
const FRAME_TYPE_SIZE: usize = 1;

const MAX_USER_AGENT_LENGTH: usize = 128;
const MAX_LOCATOR_COUNT: usize = MAX_HEADERS_PER_MESSAGE;

const MAX_FRAME_PAYLOAD_SIZE: usize = MAX_P2P_MESSAGE_SIZE;

const MAX_FRAME_SIZE: usize = FRAME_LENGTH_SIZE + FRAME_TYPE_SIZE + MAX_FRAME_PAYLOAD_SIZE;

// ================================================================
// CODEC ERROR
// ================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum P2pCodecError {
    Io(String),
    EmptyPayload,
    FrameTooLarge,
    InvalidMessageType(u8),
    TruncatedFrame,
    InvalidProtocolVersion,
    InvalidPeerId,
    InvalidString,
    InvalidCount,
    InvalidHash,
    InvalidHeight,
    InvalidMessage,
}

impl std::fmt::Display for P2pCodecError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(message) => {
                write!(formatter, "io error: {}", message)
            }

            Self::EmptyPayload => {
                write!(formatter, "empty payload")
            }

            Self::FrameTooLarge => {
                write!(formatter, "frame too large")
            }

            Self::InvalidMessageType(value) => {
                write!(formatter, "invalid message type: {}", value)
            }

            Self::TruncatedFrame => {
                write!(formatter, "truncated frame")
            }

            Self::InvalidProtocolVersion => {
                write!(formatter, "invalid protocol version")
            }

            Self::InvalidPeerId => {
                write!(formatter, "invalid peer id")
            }

            Self::InvalidString => {
                write!(formatter, "invalid string")
            }

            Self::InvalidCount => {
                write!(formatter, "invalid count")
            }

            Self::InvalidHash => {
                write!(formatter, "invalid hash")
            }

            Self::InvalidHeight => {
                write!(formatter, "invalid height")
            }

            Self::InvalidMessage => {
                write!(formatter, "invalid message")
            }
        }
    }
}

impl std::error::Error for P2pCodecError {}

impl From<io::Error> for P2pCodecError {
    fn from(error: io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

// ================================================================
// ENCODER
// ================================================================

pub struct P2pCodec;

impl P2pCodec {
    pub fn encode(message: &P2pMessage) -> Result<Vec<u8>, P2pCodecError> {
        message
            .validate_for_network()
            .map_err(|_| P2pCodecError::InvalidMessage)?;

        let payload = Self::encode_payload(message)?;

        if payload.len() > MAX_FRAME_PAYLOAD_SIZE {
            return Err(P2pCodecError::FrameTooLarge);
        }

        let payload_len_with_type = payload
            .len()
            .checked_add(FRAME_TYPE_SIZE)
            .ok_or(P2pCodecError::FrameTooLarge)?;

        let payload_len_u32 =
            u32::try_from(payload_len_with_type).map_err(|_| P2pCodecError::FrameTooLarge)?;

        let frame_len = FRAME_LENGTH_SIZE
            .checked_add(payload_len_with_type)
            .ok_or(P2pCodecError::FrameTooLarge)?;

        if frame_len > MAX_FRAME_SIZE {
            return Err(P2pCodecError::FrameTooLarge);
        }

        let mut result = Vec::with_capacity(frame_len);

        result.extend_from_slice(&payload_len_u32.to_be_bytes());

        result.push(message.message_type().code());

        result.extend_from_slice(&payload);

        Ok(result)
    }

    fn encode_payload(message: &P2pMessage) -> Result<Vec<u8>, P2pCodecError> {
        let mut output = Vec::new();

        match message {
            P2pMessage::Version {
                protocol_version,
                node_id,
                user_agent,
                start_height,
            } => {
                output.extend_from_slice(&protocol_version.to_be_bytes());

                output.extend_from_slice(node_id);

                Self::write_string(&mut output, user_agent, MAX_USER_AGENT_LENGTH)?;

                output.extend_from_slice(&start_height.to_be_bytes());
            }

            P2pMessage::VerAck => {}

            P2pMessage::Ping { nonce } => {
                output.extend_from_slice(&nonce.to_be_bytes());
            }

            P2pMessage::Pong { nonce } => {
                output.extend_from_slice(&nonce.to_be_bytes());
            }

            P2pMessage::GetHeaders { locator, stop_hash } => {
                Self::write_hashes(&mut output, locator, MAX_LOCATOR_COUNT)?;

                Self::write_optional_hash(&mut output, stop_hash);
            }

            P2pMessage::Headers { headers } => {
                let count =
                    u32::try_from(headers.len()).map_err(|_| P2pCodecError::InvalidCount)?;

                if headers.len() > MAX_HEADERS_PER_MESSAGE {
                    return Err(P2pCodecError::InvalidCount);
                }

                output.extend_from_slice(&count.to_be_bytes());

                for header in headers {
                    if header.hash == [0u8; 32] {
                        return Err(P2pCodecError::InvalidHash);
                    }

                    output.extend_from_slice(&header.hash);

                    output.extend_from_slice(&header.height.to_be_bytes());
                }
            }

            P2pMessage::GetBlocks { locator, stop_hash } => {
                Self::write_hashes(&mut output, locator, MAX_LOCATOR_COUNT)?;

                Self::write_optional_hash(&mut output, stop_hash);
            }

            P2pMessage::Blocks { hashes } => {
                Self::write_hashes(&mut output, hashes, MAX_BLOCKS_PER_MESSAGE)?;
            }

            P2pMessage::Transaction { transaction_id } => {
                if *transaction_id == [0u8; 32] {
                    return Err(P2pCodecError::InvalidHash);
                }

                output.extend_from_slice(transaction_id);
            }

            P2pMessage::Reject {
                message_type,
                reason,
            } => {
                output.push(message_type.code());

                Self::write_string(&mut output, reason, MAX_REJECT_REASON_LENGTH)?;
            }
        }

        Ok(output)
    }

    fn write_string(
        output: &mut Vec<u8>,
        value: &str,
        maximum: usize,
    ) -> Result<(), P2pCodecError> {
        if value.is_empty() {
            return Err(P2pCodecError::InvalidString);
        }

        if value.len() > maximum {
            return Err(P2pCodecError::InvalidString);
        }

        let length = u32::try_from(value.len()).map_err(|_| P2pCodecError::InvalidString)?;

        output.extend_from_slice(&length.to_be_bytes());

        output.extend_from_slice(value.as_bytes());

        Ok(())
    }

    fn write_hashes(
        output: &mut Vec<u8>,
        hashes: &[[u8; 32]],
        maximum: usize,
    ) -> Result<(), P2pCodecError> {
        if hashes.is_empty() {
            return Err(P2pCodecError::InvalidCount);
        }

        if hashes.len() > maximum {
            return Err(P2pCodecError::InvalidCount);
        }

        let count = u32::try_from(hashes.len()).map_err(|_| P2pCodecError::InvalidCount)?;

        output.extend_from_slice(&count.to_be_bytes());

        for hash in hashes {
            if *hash == [0u8; 32] {
                return Err(P2pCodecError::InvalidHash);
            }

            output.extend_from_slice(hash);
        }

        Ok(())
    }

    fn write_optional_hash(output: &mut Vec<u8>, hash: &Option<[u8; 32]>) {
        match hash {
            Some(value) => {
                output.push(1);
                output.extend_from_slice(value);
            }

            None => {
                output.push(0);
            }
        }
    }

    // ============================================================
    // DECODER
    // ============================================================

    pub fn decode(frame: &[u8]) -> Result<P2pMessage, P2pCodecError> {
        if frame.len() < FRAME_LENGTH_SIZE + FRAME_TYPE_SIZE {
            return Err(P2pCodecError::TruncatedFrame);
        }

        let declared_length = u32::from_be_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;

        if declared_length < FRAME_TYPE_SIZE {
            return Err(P2pCodecError::TruncatedFrame);
        }

        if declared_length > MAX_P2P_MESSAGE_SIZE {
            return Err(P2pCodecError::FrameTooLarge);
        }

        let expected_total = FRAME_LENGTH_SIZE
            .checked_add(declared_length)
            .ok_or(P2pCodecError::FrameTooLarge)?;

        if frame.len() != expected_total {
            return Err(P2pCodecError::TruncatedFrame);
        }

        let message_type = frame[FRAME_LENGTH_SIZE];

        let payload_start = FRAME_LENGTH_SIZE + FRAME_TYPE_SIZE;

        let payload = &frame[payload_start..];

        Self::decode_payload(message_type, payload)
    }

    fn decode_payload(message_type: u8, payload: &[u8]) -> Result<P2pMessage, P2pCodecError> {
        match message_type {
            1 => Self::decode_version(payload),

            2 => {
                if !payload.is_empty() {
                    return Err(P2pCodecError::InvalidMessage);
                }

                Ok(P2pMessage::VerAck)
            }

            3 => {
                let mut reader = ByteReader::new(payload);

                let nonce = reader.read_u64()?;

                reader.finish()?;

                Ok(P2pMessage::Ping { nonce })
            }

            4 => {
                let mut reader = ByteReader::new(payload);

                let nonce = reader.read_u64()?;

                reader.finish()?;

                Ok(P2pMessage::Pong { nonce })
            }

            5 => Self::decode_get_headers(payload),

            6 => Self::decode_headers(payload),

            7 => Self::decode_get_blocks(payload),

            8 => Self::decode_blocks(payload),

            9 => {
                if payload.len() != 32 {
                    return Err(P2pCodecError::InvalidMessage);
                }

                let mut transaction_id = [0u8; 32];

                transaction_id.copy_from_slice(payload);

                if transaction_id == [0u8; 32] {
                    return Err(P2pCodecError::InvalidHash);
                }

                let message = P2pMessage::Transaction { transaction_id };

                message
                    .validate_for_network()
                    .map_err(|_| P2pCodecError::InvalidMessage)?;

                Ok(message)
            }

            10 => Self::decode_reject(payload),

            value => Err(P2pCodecError::InvalidMessageType(value)),
        }
    }

    fn decode_version(payload: &[u8]) -> Result<P2pMessage, P2pCodecError> {
        let mut reader = ByteReader::new(payload);

        let protocol_version = reader.read_u32()?;

        if protocol_version != 1 {
            return Err(P2pCodecError::InvalidProtocolVersion);
        }

        let node_id = reader.read_peer_id()?;

        if node_id == [0u8; 32] {
            return Err(P2pCodecError::InvalidPeerId);
        }

        let user_agent = reader.read_string(MAX_USER_AGENT_LENGTH)?;

        let start_height = reader.read_u64()?;

        reader.finish()?;

        let message = P2pMessage::Version {
            protocol_version,
            node_id,
            user_agent,
            start_height,
        };

        message
            .validate_for_network()
            .map_err(|_| P2pCodecError::InvalidMessage)?;

        Ok(message)
    }

    fn decode_get_headers(payload: &[u8]) -> Result<P2pMessage, P2pCodecError> {
        let mut reader = ByteReader::new(payload);

        let locator = reader.read_hashes(MAX_HEADERS_PER_MESSAGE)?;

        let stop_hash = reader.read_optional_hash()?;

        reader.finish()?;

        let message = P2pMessage::GetHeaders { locator, stop_hash };

        message
            .validate_for_network()
            .map_err(|_| P2pCodecError::InvalidMessage)?;

        Ok(message)
    }

    fn decode_headers(payload: &[u8]) -> Result<P2pMessage, P2pCodecError> {
        let mut reader = ByteReader::new(payload);

        let count = reader.read_u32()? as usize;

        if count == 0 || count > MAX_HEADERS_PER_MESSAGE {
            return Err(P2pCodecError::InvalidCount);
        }

        let mut headers = Vec::with_capacity(count);

        for _ in 0..count {
            let hash = reader.read_hash()?;

            if hash == [0u8; 32] {
                return Err(P2pCodecError::InvalidHash);
            }

            let height = reader.read_u64()?;

            headers.push(HeaderRef { hash, height });
        }

        reader.finish()?;

        let message = P2pMessage::Headers { headers };

        message
            .validate_for_network()
            .map_err(|_| P2pCodecError::InvalidMessage)?;

        Ok(message)
    }

    fn decode_get_blocks(payload: &[u8]) -> Result<P2pMessage, P2pCodecError> {
        let mut reader = ByteReader::new(payload);

        let locator = reader.read_hashes(MAX_HEADERS_PER_MESSAGE)?;

        let stop_hash = reader.read_optional_hash()?;

        reader.finish()?;

        let message = P2pMessage::GetBlocks { locator, stop_hash };

        message
            .validate_for_network()
            .map_err(|_| P2pCodecError::InvalidMessage)?;

        Ok(message)
    }

    fn decode_blocks(payload: &[u8]) -> Result<P2pMessage, P2pCodecError> {
        let mut reader = ByteReader::new(payload);

        let hashes = reader.read_hashes(MAX_BLOCKS_PER_MESSAGE)?;

        reader.finish()?;

        let message = P2pMessage::Blocks { hashes };

        message
            .validate_for_network()
            .map_err(|_| P2pCodecError::InvalidMessage)?;

        Ok(message)
    }

    fn decode_reject(payload: &[u8]) -> Result<P2pMessage, P2pCodecError> {
        let mut reader = ByteReader::new(payload);

        let message_type_code = reader.read_u8()?;

        let message_type = match message_type_code {
            1 => P2pMessageType::Version,
            2 => P2pMessageType::VerAck,
            3 => P2pMessageType::Ping,
            4 => P2pMessageType::Pong,
            5 => P2pMessageType::GetHeaders,
            6 => P2pMessageType::Headers,
            7 => P2pMessageType::GetBlocks,
            8 => P2pMessageType::Blocks,
            9 => P2pMessageType::Transaction,
            10 => P2pMessageType::Reject,

            value => {
                return Err(P2pCodecError::InvalidMessageType(value));
            }
        };

        let reason = reader.read_string(MAX_REJECT_REASON_LENGTH)?;

        reader.finish()?;

        let message = P2pMessage::Reject {
            message_type,
            reason,
        };

        message
            .validate_for_network()
            .map_err(|_| P2pCodecError::InvalidMessage)?;

        Ok(message)
    }

    // ============================================================
    // STREAM I/O
    // ============================================================

    pub fn write_message<W: Write>(
        writer: &mut W,
        message: &P2pMessage,
    ) -> Result<(), P2pCodecError> {
        let encoded = Self::encode(message)?;

        writer.write_all(&encoded)?;

        Ok(())
    }

    pub fn read_message<R: Read>(reader: &mut R) -> Result<P2pMessage, P2pCodecError> {
        let mut length_bytes = [0u8; 4];

        reader.read_exact(&mut length_bytes)?;

        let declared_length = u32::from_be_bytes(length_bytes) as usize;

        if declared_length < 1 {
            return Err(P2pCodecError::TruncatedFrame);
        }

        if declared_length > MAX_P2P_MESSAGE_SIZE {
            return Err(P2pCodecError::FrameTooLarge);
        }

        let total_length = FRAME_LENGTH_SIZE
            .checked_add(declared_length)
            .ok_or(P2pCodecError::FrameTooLarge)?;

        if total_length > MAX_FRAME_SIZE {
            return Err(P2pCodecError::FrameTooLarge);
        }

        let mut frame = Vec::with_capacity(total_length);

        frame.extend_from_slice(&length_bytes);

        let mut rest = vec![0u8; declared_length];

        reader.read_exact(&mut rest)?;

        frame.extend_from_slice(&rest);

        Self::decode(&frame)
    }
}

// ================================================================
// BYTE READER
// ================================================================

struct ByteReader<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> ByteReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.position)
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], P2pCodecError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(P2pCodecError::TruncatedFrame)?;

        if end > self.data.len() {
            return Err(P2pCodecError::TruncatedFrame);
        }

        let result = &self.data[self.position..end];

        self.position = end;

        Ok(result)
    }

    fn read_u8(&mut self) -> Result<u8, P2pCodecError> {
        Ok(self.take(1)?[0])
    }

    fn read_u32(&mut self) -> Result<u32, P2pCodecError> {
        let bytes = self.take(4)?;

        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u64(&mut self) -> Result<u64, P2pCodecError> {
        let bytes = self.take(8)?;

        Ok(u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_hash(&mut self) -> Result<[u8; 32], P2pCodecError> {
        let bytes = self.take(32)?;

        let mut hash = [0u8; 32];

        hash.copy_from_slice(bytes);

        Ok(hash)
    }

    fn read_peer_id(&mut self) -> Result<PeerId, P2pCodecError> {
        self.read_hash()
    }

    fn read_string(&mut self, maximum: usize) -> Result<String, P2pCodecError> {
        let length = self.read_u32()? as usize;

        if length == 0 || length > maximum {
            return Err(P2pCodecError::InvalidString);
        }

        let bytes = self.take(length)?;

        String::from_utf8(bytes.to_vec()).map_err(|_| P2pCodecError::InvalidString)
    }

    fn read_hashes(&mut self, maximum: usize) -> Result<Vec<[u8; 32]>, P2pCodecError> {
        let count = self.read_u32()? as usize;

        if count == 0 || count > maximum {
            return Err(P2pCodecError::InvalidCount);
        }

        let mut result = Vec::with_capacity(count);

        for _ in 0..count {
            let hash = self.read_hash()?;

            if hash == [0u8; 32] {
                return Err(P2pCodecError::InvalidHash);
            }

            result.push(hash);
        }

        Ok(result)
    }

    fn read_optional_hash(&mut self) -> Result<Option<[u8; 32]>, P2pCodecError> {
        match self.read_u8()? {
            0 => Ok(None),

            1 => {
                let hash = self.read_hash()?;

                if hash == [0u8; 32] {
                    return Err(P2pCodecError::InvalidHash);
                }

                Ok(Some(hash))
            }

            _ => Err(P2pCodecError::InvalidMessage),
        }
    }

    fn finish(&self) -> Result<(), P2pCodecError> {
        if self.remaining() != 0 {
            return Err(P2pCodecError::InvalidMessage);
        }

        Ok(())
    }
}

// ================================================================
// TESTS
// ================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn peer_id(value: u8) -> PeerId {
        [value; 32]
    }

    fn hash(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn round_trip(message: P2pMessage) {
        let encoded = P2pCodec::encode(&message).expect("encode");

        let decoded = P2pCodec::decode(&encoded).expect("decode");

        assert_eq!(message, decoded);
    }

    // ============================================================
    // NORMAL ROUND-TRIP TESTS
    // ============================================================

    #[test]
    fn version_round_trip() {
        round_trip(P2pMessage::Version {
            protocol_version: 1,
            node_id: peer_id(1),
            user_agent: "/nio:0.1.0/".to_string(),
            start_height: 10,
        });
    }

    #[test]
    fn verack_round_trip() {
        round_trip(P2pMessage::VerAck);
    }

    #[test]
    fn ping_round_trip() {
        round_trip(P2pMessage::Ping { nonce: 12345 });
    }

    #[test]
    fn pong_round_trip() {
        round_trip(P2pMessage::Pong { nonce: 98765 });
    }

    #[test]
    fn getheaders_round_trip() {
        round_trip(P2pMessage::GetHeaders {
            locator: vec![hash(1), hash(2), hash(3)],
            stop_hash: Some(hash(4)),
        });
    }

    #[test]
    fn headers_round_trip() {
        round_trip(P2pMessage::Headers {
            headers: vec![
                HeaderRef {
                    hash: hash(1),
                    height: 1,
                },
                HeaderRef {
                    hash: hash(2),
                    height: 2,
                },
            ],
        });
    }

    #[test]
    fn getblocks_round_trip() {
        round_trip(P2pMessage::GetBlocks {
            locator: vec![hash(1), hash(2)],
            stop_hash: None,
        });
    }

    #[test]
    fn blocks_round_trip() {
        round_trip(P2pMessage::Blocks {
            hashes: vec![hash(1), hash(2), hash(3)],
        });
    }

    #[test]
    fn transaction_round_trip() {
        round_trip(P2pMessage::Transaction {
            transaction_id: hash(9),
        });
    }

    #[test]
    fn reject_round_trip() {
        round_trip(P2pMessage::Reject {
            message_type: P2pMessageType::Transaction,
            reason: "invalid transaction".to_string(),
        });
    }

    // ============================================================
    // NORMAL VALIDATION TESTS
    // ============================================================

    #[test]
    fn encoded_message_has_length_prefix() {
        let message = P2pMessage::Ping { nonce: 42 };

        let encoded = P2pCodec::encode(&message).expect("encode");

        assert!(encoded.len() >= 5);

        let declared =
            u32::from_be_bytes([encoded[0], encoded[1], encoded[2], encoded[3]]) as usize;

        assert_eq!(declared + 4, encoded.len());
    }

    #[test]
    fn invalid_message_type_is_rejected() {
        let frame = vec![0, 0, 0, 1, 255];

        assert!(matches!(
            P2pCodec::decode(&frame),
            Err(P2pCodecError::InvalidMessageType(255))
        ));
    }

    #[test]
    fn truncated_frame_is_rejected() {
        let frame = vec![0, 0, 0, 9, 3];

        assert!(P2pCodec::decode(&frame).is_err());
    }

    #[test]
    fn oversized_frame_is_rejected() {
        let length = (MAX_P2P_MESSAGE_SIZE + 1) as u32;

        let bytes = length.to_be_bytes();

        let frame = vec![bytes[0], bytes[1], bytes[2], bytes[3], 3];

        assert!(matches!(
            P2pCodec::decode(&frame),
            Err(P2pCodecError::FrameTooLarge)
        ));
    }

    #[test]
    fn trailing_bytes_are_rejected() {
        let message = P2pMessage::Ping { nonce: 42 };

        let mut encoded = P2pCodec::encode(&message).expect("encode");

        encoded.push(99);

        assert!(P2pCodec::decode(&encoded).is_err());
    }

    #[test]
    fn stream_write_and_read_work() {
        let message = P2pMessage::Ping { nonce: 123 };

        let mut buffer = Vec::new();

        P2pCodec::write_message(&mut buffer, &message).expect("write");

        let mut cursor = Cursor::new(buffer);

        let decoded = P2pCodec::read_message(&mut cursor).expect("read");

        assert_eq!(message, decoded);
    }

    #[test]
    fn invalid_optional_hash_flag_is_rejected() {
        let message = P2pMessage::GetHeaders {
            locator: vec![hash(1)],
            stop_hash: None,
        };

        let mut encoded = P2pCodec::encode(&message).expect("encode");

        let last = encoded.len() - 1;

        encoded[last] = 9;

        assert!(P2pCodec::decode(&encoded).is_err());
    }

    #[test]
    fn zero_transaction_id_is_rejected() {
        let message = P2pMessage::Transaction {
            transaction_id: [0u8; 32],
        };

        assert!(P2pCodec::encode(&message).is_err());
    }

    #[test]
    fn excessive_header_count_is_rejected() {
        let message = P2pMessage::Headers {
            headers: vec![
                HeaderRef {
                    hash: hash(1),
                    height: 1,
                };
                MAX_HEADERS_PER_MESSAGE + 1
            ],
        };

        assert!(P2pCodec::encode(&message).is_err());
    }

    #[test]
    fn excessive_block_count_is_rejected() {
        let message = P2pMessage::Blocks {
            hashes: vec![hash(1); MAX_BLOCKS_PER_MESSAGE + 1],
        };

        assert!(P2pCodec::encode(&message).is_err());
    }

    // ============================================================
    // ATTACK / FAILURE TESTS
    // ============================================================

    #[test]
    fn attack_invalid_message_type_is_rejected() {
        let frame = vec![0, 0, 0, 1, 255];

        let result = P2pCodec::decode(&frame);

        assert!(matches!(
            result,
            Err(P2pCodecError::InvalidMessageType(255))
        ));
    }

    #[test]
    fn attack_zero_length_frame_is_rejected() {
        let frame = vec![0, 0, 0, 0];

        let result = P2pCodec::decode(&frame);

        assert!(matches!(result, Err(P2pCodecError::TruncatedFrame)));
    }

    #[test]
    fn attack_declared_length_smaller_than_actual_is_rejected() {
        let message = P2pMessage::Ping { nonce: 42 };

        let mut frame = P2pCodec::encode(&message).expect("encode");

        frame[3] = frame[3].saturating_sub(1);

        let result = P2pCodec::decode(&frame);

        assert!(result.is_err());
    }

    #[test]
    fn attack_declared_length_larger_than_actual_is_rejected() {
        let frame = vec![0, 0, 0, 100, 3];

        let result = P2pCodec::decode(&frame);

        assert!(result.is_err());
    }

    #[test]
    fn attack_trailing_bytes_are_rejected() {
        let message = P2pMessage::Ping { nonce: 42 };

        let mut frame = P2pCodec::encode(&message).expect("encode");

        frame.extend_from_slice(&[1, 2, 3, 4, 5]);

        let result = P2pCodec::decode(&frame);

        assert!(result.is_err());
    }

    #[test]
    fn attack_zero_hash_is_rejected() {
        let message = P2pMessage::Transaction {
            transaction_id: [0u8; 32],
        };

        let result = P2pCodec::encode(&message);

        assert!(result.is_err());
    }

    #[test]
    fn attack_invalid_optional_hash_flag_is_rejected() {
        let message = P2pMessage::GetHeaders {
            locator: vec![hash(1)],
            stop_hash: None,
        };

        let mut frame = P2pCodec::encode(&message).expect("encode");

        let last = frame.len() - 1;

        frame[last] = 255;

        let result = P2pCodec::decode(&frame);

        assert!(result.is_err());
    }

    #[test]
    fn attack_empty_hash_list_is_rejected() {
        let mut payload = Vec::new();

        payload.extend_from_slice(&0u32.to_be_bytes());

        payload.push(0);

        let declared = (payload.len() + 1) as u32;

        let mut frame = Vec::new();

        frame.extend_from_slice(&declared.to_be_bytes());

        frame.push(5);

        frame.extend_from_slice(&payload);

        let result = P2pCodec::decode(&frame);

        assert!(result.is_err());
    }

    #[test]
    fn attack_zero_header_count_is_rejected() {
        let mut payload = Vec::new();

        payload.extend_from_slice(&0u32.to_be_bytes());

        let declared = (payload.len() + 1) as u32;

        let mut frame = Vec::new();

        frame.extend_from_slice(&declared.to_be_bytes());

        frame.push(6);

        frame.extend_from_slice(&payload);

        let result = P2pCodec::decode(&frame);

        assert!(result.is_err());
    }

    #[test]
    fn attack_zero_block_hash_count_is_rejected() {
        let mut payload = Vec::new();

        payload.extend_from_slice(&0u32.to_be_bytes());

        let declared = (payload.len() + 1) as u32;

        let mut frame = Vec::new();

        frame.extend_from_slice(&declared.to_be_bytes());

        frame.push(8);

        frame.extend_from_slice(&payload);

        let result = P2pCodec::decode(&frame);

        assert!(result.is_err());
    }

    #[test]
    fn attack_invalid_utf8_user_agent_is_rejected() {
        let mut payload = Vec::new();

        payload.extend_from_slice(&1u32.to_be_bytes());

        payload.extend_from_slice(&peer_id(1));

        payload.extend_from_slice(&2u32.to_be_bytes());

        payload.extend_from_slice(&[0xff, 0xff]);

        payload.extend_from_slice(&0u64.to_be_bytes());

        let declared = (payload.len() + 1) as u32;

        let mut frame = Vec::new();

        frame.extend_from_slice(&declared.to_be_bytes());

        frame.push(1);

        frame.extend_from_slice(&payload);

        let result = P2pCodec::decode(&frame);

        assert!(matches!(result, Err(P2pCodecError::InvalidString)));
    }

    #[test]
    fn attack_invalid_protocol_version_is_rejected() {
        let mut payload = Vec::new();

        payload.extend_from_slice(&999u32.to_be_bytes());

        payload.extend_from_slice(&peer_id(1));

        payload.extend_from_slice(&1u32.to_be_bytes());

        payload.push(b'x');

        payload.extend_from_slice(&0u64.to_be_bytes());

        let declared = (payload.len() + 1) as u32;

        let mut frame = Vec::new();

        frame.extend_from_slice(&declared.to_be_bytes());

        frame.push(1);

        frame.extend_from_slice(&payload);

        let result = P2pCodec::decode(&frame);

        assert!(matches!(result, Err(P2pCodecError::InvalidProtocolVersion)));
    }

    #[test]
    fn attack_zero_peer_id_is_rejected() {
        let mut payload = Vec::new();

        payload.extend_from_slice(&1u32.to_be_bytes());

        payload.extend_from_slice(&[0u8; 32]);

        payload.extend_from_slice(&1u32.to_be_bytes());

        payload.push(b'x');

        payload.extend_from_slice(&0u64.to_be_bytes());

        let declared = (payload.len() + 1) as u32;

        let mut frame = Vec::new();

        frame.extend_from_slice(&declared.to_be_bytes());

        frame.push(1);

        frame.extend_from_slice(&payload);

        let result = P2pCodec::decode(&frame);

        assert!(matches!(result, Err(P2pCodecError::InvalidPeerId)));
    }

    #[test]
    fn attack_truncated_version_message_is_rejected() {
        let payload = vec![0u8; 10];

        let declared = (payload.len() + 1) as u32;

        let mut frame = Vec::new();

        frame.extend_from_slice(&declared.to_be_bytes());

        frame.push(1);

        frame.extend_from_slice(&payload);

        let result = P2pCodec::decode(&frame);

        assert!(result.is_err());
    }

    #[test]
    fn attack_truncated_ping_message_is_rejected() {
        let payload = vec![0u8; 7];

        let declared = (payload.len() + 1) as u32;

        let mut frame = Vec::new();

        frame.extend_from_slice(&declared.to_be_bytes());

        frame.push(3);

        frame.extend_from_slice(&payload);

        let result = P2pCodec::decode(&frame);

        assert!(result.is_err());
    }

    #[test]
    fn attack_truncated_transaction_message_is_rejected() {
        let payload = vec![1u8; 31];

        let declared = (payload.len() + 1) as u32;

        let mut frame = Vec::new();

        frame.extend_from_slice(&declared.to_be_bytes());

        frame.push(9);

        frame.extend_from_slice(&payload);

        let result = P2pCodec::decode(&frame);

        assert!(result.is_err());
    }

    #[test]
    fn attack_truncated_reject_message_is_rejected() {
        let payload = vec![10u8];

        let declared = (payload.len() + 1) as u32;

        let mut frame = Vec::new();

        frame.extend_from_slice(&declared.to_be_bytes());

        frame.push(10);

        frame.extend_from_slice(&payload);

        let result = P2pCodec::decode(&frame);

        assert!(result.is_err());
    }

    #[test]
    fn attack_invalid_reject_message_type_is_rejected() {
        let mut payload = Vec::new();

        payload.push(255);

        payload.extend_from_slice(&3u32.to_be_bytes());

        payload.extend_from_slice(b"bad");

        let declared = (payload.len() + 1) as u32;

        let mut frame = Vec::new();

        frame.extend_from_slice(&declared.to_be_bytes());

        frame.push(10);

        frame.extend_from_slice(&payload);

        let result = P2pCodec::decode(&frame);

        assert!(matches!(
            result,
            Err(P2pCodecError::InvalidMessageType(255))
        ));
    }

    #[test]
    fn attack_empty_user_agent_is_rejected() {
        let message = P2pMessage::Version {
            protocol_version: 1,
            node_id: peer_id(1),
            user_agent: String::new(),
            start_height: 0,
        };

        assert!(P2pCodec::encode(&message).is_err());
    }

    #[test]
    fn attack_oversized_user_agent_is_rejected() {
        let message = P2pMessage::Version {
            protocol_version: 1,
            node_id: peer_id(1),
            user_agent: "x".repeat(MAX_USER_AGENT_LENGTH + 1),
            start_height: 0,
        };

        assert!(P2pCodec::encode(&message).is_err());
    }

    #[test]
    fn attack_oversized_reject_reason_is_rejected() {
        let message = P2pMessage::Reject {
            message_type: P2pMessageType::Transaction,
            reason: "x".repeat(MAX_REJECT_REASON_LENGTH + 1),
        };

        assert!(P2pCodec::encode(&message).is_err());
    }

    #[test]
    fn attack_stream_truncated_frame_is_rejected() {
        let bytes = vec![0, 0, 0, 20, 3, 1, 2];

        let mut cursor = Cursor::new(bytes);

        let result = P2pCodec::read_message(&mut cursor);

        assert!(matches!(
            result,
            Err(P2pCodecError::Io(_)) | Err(P2pCodecError::TruncatedFrame)
        ));
    }

    #[test]
    fn attack_empty_verack_payload_is_required() {
        let message = P2pMessage::VerAck;

        let mut frame = P2pCodec::encode(&message).expect("encode");

        frame.push(1);

        assert!(P2pCodec::decode(&frame).is_err());
    }

    #[test]
    fn attack_ping_payload_size_is_rejected() {
        let mut payload = vec![0u8; 9];

        let declared = (payload.len() + 1) as u32;

        let mut frame = Vec::new();

        frame.extend_from_slice(&declared.to_be_bytes());

        frame.push(3);

        frame.append(&mut payload);

        let result = P2pCodec::decode(&frame);

        assert!(result.is_err());
    }

    #[test]
    fn attack_pong_payload_size_is_rejected() {
        let mut payload = vec![0u8; 9];

        let declared = (payload.len() + 1) as u32;

        let mut frame = Vec::new();

        frame.extend_from_slice(&declared.to_be_bytes());

        frame.push(4);

        frame.append(&mut payload);

        let result = P2pCodec::decode(&frame);

        assert!(result.is_err());
    }
}
