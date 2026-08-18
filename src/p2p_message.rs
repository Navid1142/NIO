use crate::p2p::PeerId;

// ================================================================
// NIO P2P MESSAGE PROTOCOL
// ================================================================
//
// This module defines the logical message types exchanged between
// NIO peers.
//
// Serialization / wire encoding is intentionally NOT implemented
// here yet. This stage establishes:
//   - protocol message types
//   - message validation
//   - message size limits
//   - safe message metadata
//
// ================================================================

// ----------------------------------------------------------------
// PROTOCOL CONSTANTS
// ----------------------------------------------------------------

pub const P2P_MESSAGE_PROTOCOL_VERSION: u32 = 1;

pub const MAX_P2P_MESSAGE_SIZE: usize = 2_000_000;

pub const MAX_HEADERS_PER_MESSAGE: usize = 2_000;

pub const MAX_BLOCKS_PER_MESSAGE: usize = 128;

pub const MAX_TRANSACTION_BATCH_SIZE: usize = 1_000;

pub const MAX_REJECT_REASON_LENGTH: usize = 256;

// ----------------------------------------------------------------
// MESSAGE TYPE
// ----------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum P2pMessageType {
    Version,
    VerAck,
    Ping,
    Pong,
    GetHeaders,
    Headers,
    GetBlocks,
    Blocks,
    Transaction,
    Reject,
}

impl P2pMessageType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Version => "version",
            Self::VerAck => "verack",
            Self::Ping => "ping",
            Self::Pong => "pong",
            Self::GetHeaders => "getheaders",
            Self::Headers => "headers",
            Self::GetBlocks => "getblocks",
            Self::Blocks => "blocks",
            Self::Transaction => "tx",
            Self::Reject => "reject",
        }
    }

    pub fn code(&self) -> u8 {
        match self {
            Self::Version => 1,
            Self::VerAck => 2,
            Self::Ping => 3,
            Self::Pong => 4,
            Self::GetHeaders => 5,
            Self::Headers => 6,
            Self::GetBlocks => 7,
            Self::Blocks => 8,
            Self::Transaction => 9,
            Self::Reject => 10,
        }
    }
}

// ----------------------------------------------------------------
// BLOCK HEADER REFERENCE
// ----------------------------------------------------------------
//
// We intentionally transmit only the identifying information here.
// Full Block objects will be handled by the block synchronization
// layer in a later stage.
//
// ----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderRef {
    pub hash: [u8; 32],
    pub height: u64,
}

// ----------------------------------------------------------------
// MESSAGE
// ----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum P2pMessage {
    Version {
        protocol_version: u32,
        node_id: PeerId,
        user_agent: String,
        start_height: u64,
    },

    VerAck,

    Ping {
        nonce: u64,
    },

    Pong {
        nonce: u64,
    },

    GetHeaders {
        locator: Vec<[u8; 32]>,
        stop_hash: Option<[u8; 32]>,
    },

    Headers {
        headers: Vec<HeaderRef>,
    },

    GetBlocks {
        locator: Vec<[u8; 32]>,
        stop_hash: Option<[u8; 32]>,
    },

    Blocks {
        hashes: Vec<[u8; 32]>,
    },

    Transaction {
        transaction_id: [u8; 32],
    },

    Reject {
        message_type: P2pMessageType,
        reason: String,
    },
}

impl P2pMessage {
    // ============================================================
    // MESSAGE TYPE
    // ============================================================

    pub fn message_type(&self) -> P2pMessageType {
        match self {
            Self::Version { .. } => P2pMessageType::Version,
            Self::VerAck => P2pMessageType::VerAck,
            Self::Ping { .. } => P2pMessageType::Ping,
            Self::Pong { .. } => P2pMessageType::Pong,
            Self::GetHeaders { .. } => P2pMessageType::GetHeaders,
            Self::Headers { .. } => P2pMessageType::Headers,
            Self::GetBlocks { .. } => P2pMessageType::GetBlocks,
            Self::Blocks { .. } => P2pMessageType::Blocks,
            Self::Transaction { .. } => P2pMessageType::Transaction,
            Self::Reject { .. } => P2pMessageType::Reject,
        }
    }

    pub fn message_name(&self) -> &'static str {
        self.message_type().as_str()
    }

    // ============================================================
    // ESTIMATED SIZE
    // ============================================================

    pub fn estimated_size(&self) -> usize {
        match self {
            Self::Version { user_agent, .. } => 4 + 32 + 8 + user_agent.len(),

            Self::VerAck => 1,

            Self::Ping { .. } => 8,

            Self::Pong { .. } => 8,

            Self::GetHeaders { locator, stop_hash } => {
                4 + locator.len() * 32 + if stop_hash.is_some() { 32 } else { 0 }
            }

            Self::Headers { headers } => 4 + headers.len() * (32 + 8),

            Self::GetBlocks { locator, stop_hash } => {
                4 + locator.len() * 32 + if stop_hash.is_some() { 32 } else { 0 }
            }

            Self::Blocks { hashes } => 4 + hashes.len() * 32,

            Self::Transaction { .. } => 32,

            Self::Reject { reason, .. } => 1 + 4 + reason.len(),
        }
    }

    // ============================================================
    // BASIC VALIDATION
    // ============================================================

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Version {
                protocol_version,
                node_id,
                user_agent,
                ..
            } => {
                if *protocol_version != P2P_MESSAGE_PROTOCOL_VERSION {
                    return Err("unsupported p2p message protocol version".to_string());
                }

                if *node_id == [0u8; 32] {
                    return Err("version message contains zero node id".to_string());
                }

                if user_agent.is_empty() {
                    return Err("version message user agent cannot be empty".to_string());
                }

                if user_agent.len() > 128 {
                    return Err("version message user agent is too long".to_string());
                }

                Ok(())
            }

            Self::VerAck => Ok(()),

            Self::Ping { .. } => Ok(()),

            Self::Pong { .. } => Ok(()),

            Self::GetHeaders { locator, .. } => {
                if locator.is_empty() {
                    return Err("getheaders locator cannot be empty".to_string());
                }

                if locator.len() > MAX_HEADERS_PER_MESSAGE {
                    return Err("getheaders locator exceeds maximum size".to_string());
                }

                Ok(())
            }

            Self::Headers { headers } => {
                if headers.is_empty() {
                    return Err("headers message cannot be empty".to_string());
                }

                if headers.len() > MAX_HEADERS_PER_MESSAGE {
                    return Err("headers message exceeds maximum size".to_string());
                }

                for header in headers {
                    if header.hash == [0u8; 32] {
                        return Err("headers message contains zero hash".to_string());
                    }
                }

                Ok(())
            }

            Self::GetBlocks { locator, .. } => {
                if locator.is_empty() {
                    return Err("getblocks locator cannot be empty".to_string());
                }

                if locator.len() > MAX_HEADERS_PER_MESSAGE {
                    return Err("getblocks locator exceeds maximum size".to_string());
                }

                Ok(())
            }

            Self::Blocks { hashes } => {
                if hashes.is_empty() {
                    return Err("blocks message cannot be empty".to_string());
                }

                if hashes.len() > MAX_BLOCKS_PER_MESSAGE {
                    return Err("blocks message exceeds maximum size".to_string());
                }

                for hash in hashes {
                    if *hash == [0u8; 32] {
                        return Err("blocks message contains zero hash".to_string());
                    }
                }

                Ok(())
            }

            Self::Transaction { transaction_id } => {
                if *transaction_id == [0u8; 32] {
                    return Err("transaction id cannot be zero".to_string());
                }

                Ok(())
            }

            Self::Reject { reason, .. } => {
                if reason.is_empty() {
                    return Err("reject reason cannot be empty".to_string());
                }

                if reason.len() > MAX_REJECT_REASON_LENGTH {
                    return Err("reject reason is too long".to_string());
                }

                Ok(())
            }
        }
    }

    // ============================================================
    // NETWORK SIZE VALIDATION
    // ============================================================

    pub fn is_within_size_limit(&self) -> bool {
        self.estimated_size() <= MAX_P2P_MESSAGE_SIZE
    }

    pub fn validate_for_network(&self) -> Result<(), String> {
        self.validate()?;

        if !self.is_within_size_limit() {
            return Err("p2p message exceeds maximum network size".to_string());
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

    fn node_id(value: u8) -> PeerId {
        [value; 32]
    }

    fn hash(value: u8) -> [u8; 32] {
        [value; 32]
    }

    // ------------------------------------------------------------
    // MESSAGE TYPES
    // ------------------------------------------------------------

    #[test]
    fn message_type_codes_are_stable() {
        assert_eq!(P2pMessageType::Version.code(), 1);

        assert_eq!(P2pMessageType::VerAck.code(), 2);

        assert_eq!(P2pMessageType::Ping.code(), 3);

        assert_eq!(P2pMessageType::Pong.code(), 4);

        assert_eq!(P2pMessageType::GetHeaders.code(), 5);

        assert_eq!(P2pMessageType::Headers.code(), 6);

        assert_eq!(P2pMessageType::GetBlocks.code(), 7);

        assert_eq!(P2pMessageType::Blocks.code(), 8);

        assert_eq!(P2pMessageType::Transaction.code(), 9);

        assert_eq!(P2pMessageType::Reject.code(), 10);
    }

    // ------------------------------------------------------------
    // VERSION
    // ------------------------------------------------------------

    #[test]
    fn valid_version_message_is_accepted() {
        let message = P2pMessage::Version {
            protocol_version: P2P_MESSAGE_PROTOCOL_VERSION,
            node_id: node_id(1),
            user_agent: "/nio:0.1.0/".to_string(),
            start_height: 0,
        };

        assert!(message.validate_for_network().is_ok());
        assert_eq!(message.message_type(), P2pMessageType::Version);
    }

    #[test]
    fn zero_node_id_is_rejected() {
        let message = P2pMessage::Version {
            protocol_version: P2P_MESSAGE_PROTOCOL_VERSION,
            node_id: [0u8; 32],
            user_agent: "/nio:0.1.0/".to_string(),
            start_height: 0,
        };

        assert!(message.validate().is_err());
    }

    #[test]
    fn incompatible_message_protocol_is_rejected() {
        let message = P2pMessage::Version {
            protocol_version: P2P_MESSAGE_PROTOCOL_VERSION + 1,
            node_id: node_id(1),
            user_agent: "/nio:0.1.0/".to_string(),
            start_height: 0,
        };

        assert!(message.validate().is_err());
    }

    // ------------------------------------------------------------
    // PING / PONG
    // ------------------------------------------------------------

    #[test]
    fn ping_is_valid() {
        let message = P2pMessage::Ping { nonce: 100 };

        assert!(message.validate_for_network().is_ok());
        assert_eq!(message.message_name(), "ping");
    }

    #[test]
    fn pong_is_valid() {
        let message = P2pMessage::Pong { nonce: 100 };

        assert!(message.validate_for_network().is_ok());
    }

    // ------------------------------------------------------------
    // HEADERS
    // ------------------------------------------------------------

    #[test]
    fn getheaders_is_valid() {
        let message = P2pMessage::GetHeaders {
            locator: vec![hash(1), hash(2)],
            stop_hash: None,
        };

        assert!(message.validate_for_network().is_ok());
    }

    #[test]
    fn empty_getheaders_is_rejected() {
        let message = P2pMessage::GetHeaders {
            locator: Vec::new(),
            stop_hash: None,
        };

        assert!(message.validate().is_err());
    }

    #[test]
    fn headers_message_is_valid() {
        let message = P2pMessage::Headers {
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
        };

        assert!(message.validate_for_network().is_ok());
    }

    #[test]
    fn zero_header_hash_is_rejected() {
        let message = P2pMessage::Headers {
            headers: vec![HeaderRef {
                hash: [0u8; 32],
                height: 1,
            }],
        };

        assert!(message.validate().is_err());
    }

    // ------------------------------------------------------------
    // BLOCKS
    // ------------------------------------------------------------

    #[test]
    fn getblocks_is_valid() {
        let message = P2pMessage::GetBlocks {
            locator: vec![hash(1)],
            stop_hash: Some(hash(2)),
        };

        assert!(message.validate_for_network().is_ok());
    }

    #[test]
    fn empty_getblocks_is_rejected() {
        let message = P2pMessage::GetBlocks {
            locator: Vec::new(),
            stop_hash: None,
        };

        assert!(message.validate().is_err());
    }

    #[test]
    fn blocks_message_is_valid() {
        let message = P2pMessage::Blocks {
            hashes: vec![hash(1), hash(2)],
        };

        assert!(message.validate_for_network().is_ok());
    }

    #[test]
    fn empty_blocks_message_is_rejected() {
        let message = P2pMessage::Blocks { hashes: Vec::new() };

        assert!(message.validate().is_err());
    }

    #[test]
    fn zero_block_hash_is_rejected() {
        let message = P2pMessage::Blocks {
            hashes: vec![[0u8; 32]],
        };

        assert!(message.validate().is_err());
    }

    // ------------------------------------------------------------
    // TRANSACTION
    // ------------------------------------------------------------

    #[test]
    fn transaction_message_is_valid() {
        let message = P2pMessage::Transaction {
            transaction_id: hash(9),
        };

        assert!(message.validate_for_network().is_ok());
    }

    #[test]
    fn zero_transaction_id_is_rejected() {
        let message = P2pMessage::Transaction {
            transaction_id: [0u8; 32],
        };

        assert!(message.validate().is_err());
    }

    // ------------------------------------------------------------
    // REJECT
    // ------------------------------------------------------------

    #[test]
    fn reject_message_is_valid() {
        let message = P2pMessage::Reject {
            message_type: P2pMessageType::Transaction,
            reason: "invalid transaction".to_string(),
        };

        assert!(message.validate_for_network().is_ok());
    }

    #[test]
    fn empty_reject_reason_is_rejected() {
        let message = P2pMessage::Reject {
            message_type: P2pMessageType::Transaction,
            reason: String::new(),
        };

        assert!(message.validate().is_err());
    }

    // ------------------------------------------------------------
    // SIZE
    // ------------------------------------------------------------

    #[test]
    fn message_size_is_bounded() {
        let message = P2pMessage::Ping { nonce: 1 };

        assert!(message.is_within_size_limit());

        assert!(message.estimated_size() < 1_000);
    }

    #[test]
    fn excessive_headers_are_rejected() {
        let message = P2pMessage::Headers {
            headers: vec![
                HeaderRef {
                    hash: hash(1),
                    height: 1,
                };
                MAX_HEADERS_PER_MESSAGE + 1
            ],
        };

        assert!(message.validate().is_err());
    }

    #[test]
    fn excessive_blocks_are_rejected() {
        let message = P2pMessage::Blocks {
            hashes: vec![hash(1); MAX_BLOCKS_PER_MESSAGE + 1],
        };

        assert!(message.validate().is_err());
    }

    #[test]
    fn excessive_reject_reason_is_rejected() {
        let message = P2pMessage::Reject {
            message_type: P2pMessageType::Ping,
            reason: "x".repeat(MAX_REJECT_REASON_LENGTH + 1),
        };

        assert!(message.validate().is_err());
    }
}
