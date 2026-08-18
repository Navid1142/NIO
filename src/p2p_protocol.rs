use crate::p2p::PeerId;

pub const P2P_PROTOCOL_VERSION: u32 = 1;

pub const MAX_MESSAGE_SIZE: usize = 2 * 1024 * 1024;
pub const MAX_USER_AGENT_LENGTH: usize = 128;

pub const MAX_GET_HEADERS: usize = 2000;
pub const MAX_GET_BLOCKS: usize = 2000;
pub const MAX_PEER_ADDRESSES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    Ping = 1,
    Pong = 2,
    Handshake = 3,
    GetPeers = 4,
    Peers = 5,
    GetHeaders = 6,
    Headers = 7,
    GetBlocks = 8,
    Blocks = 9,
    Transaction = 10,
    Block = 11,
}

impl MessageType {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Ping),
            2 => Some(Self::Pong),
            3 => Some(Self::Handshake),
            4 => Some(Self::GetPeers),
            5 => Some(Self::Peers),
            6 => Some(Self::GetHeaders),
            7 => Some(Self::Headers),
            8 => Some(Self::GetBlocks),
            9 => Some(Self::Blocks),
            10 => Some(Self::Transaction),
            11 => Some(Self::Block),
            _ => None,
        }
    }

    pub fn is_request(self) -> bool {
        matches!(
            self,
            Self::Ping
                | Self::Handshake
                | Self::GetPeers
                | Self::GetHeaders
                | Self::GetBlocks
                | Self::Transaction
                | Self::Block
        )
    }

    pub fn is_response(self) -> bool {
        matches!(
            self,
            Self::Pong | Self::Peers | Self::Headers | Self::Blocks
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolMessage {
    pub version: u32,
    pub message_type: MessageType,
    pub payload: Vec<u8>,
}

impl ProtocolMessage {
    pub fn new(message_type: MessageType, payload: Vec<u8>) -> Result<Self, String> {
        if payload.len() > MAX_MESSAGE_SIZE {
            return Err("message payload exceeds maximum size".to_string());
        }

        Ok(Self {
            version: P2P_PROTOCOL_VERSION,
            message_type,
            payload,
        })
    }

    pub fn empty(message_type: MessageType) -> Result<Self, String> {
        Self::new(message_type, Vec::new())
    }

    pub fn payload_len(&self) -> usize {
        self.payload.len()
    }

    pub fn is_empty(&self) -> bool {
        self.payload.is_empty()
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != P2P_PROTOCOL_VERSION {
            return Err("unsupported p2p protocol version".to_string());
        }

        if self.payload.len() > MAX_MESSAGE_SIZE {
            return Err("message payload exceeds maximum size".to_string());
        }

        validate_message_payload(self.message_type, &self.payload)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolHeader {
    pub version: u32,
    pub message_type: MessageType,
    pub payload_length: u32,
}

impl ProtocolHeader {
    pub fn new(
        version: u32,
        message_type: MessageType,
        payload_length: usize,
    ) -> Result<Self, String> {
        if version != P2P_PROTOCOL_VERSION {
            return Err("unsupported p2p protocol version".to_string());
        }

        if payload_length > MAX_MESSAGE_SIZE {
            return Err("payload exceeds maximum protocol size".to_string());
        }

        let payload_length =
            u32::try_from(payload_length).map_err(|_| "payload length exceeds u32".to_string())?;

        Ok(Self {
            version,
            message_type,
            payload_length,
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != P2P_PROTOCOL_VERSION {
            return Err("unsupported p2p protocol version".to_string());
        }

        if self.payload_length as usize > MAX_MESSAGE_SIZE {
            return Err("payload exceeds maximum protocol size".to_string());
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolState {
    Connected,
    HandshakePending,
    Established,
    Closing,
    Closed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolSession {
    local_node_id: PeerId,
    remote_node_id: Option<PeerId>,
    state: ProtocolState,
    received_messages: u64,
    sent_messages: u64,
}

impl ProtocolSession {
    pub fn new(local_node_id: PeerId) -> Result<Self, String> {
        if local_node_id == [0u8; 32] {
            return Err("local node id cannot be zero".to_string());
        }

        Ok(Self {
            local_node_id,
            remote_node_id: None,
            state: ProtocolState::Connected,
            received_messages: 0,
            sent_messages: 0,
        })
    }

    pub fn local_node_id(&self) -> PeerId {
        self.local_node_id
    }

    pub fn remote_node_id(&self) -> Option<PeerId> {
        self.remote_node_id
    }

    pub fn state(&self) -> ProtocolState {
        self.state
    }

    pub fn received_messages(&self) -> u64 {
        self.received_messages
    }

    pub fn sent_messages(&self) -> u64 {
        self.sent_messages
    }

    pub fn set_remote_node_id(&mut self, remote_node_id: PeerId) -> Result<(), String> {
        if remote_node_id == [0u8; 32] {
            return Err("remote node id cannot be zero".to_string());
        }

        if remote_node_id == self.local_node_id {
            self.state = ProtocolState::Failed;

            return Err("self connection is not allowed".to_string());
        }

        if self.state == ProtocolState::Closed {
            return Err("protocol session is closed".to_string());
        }

        if self.state == ProtocolState::Failed {
            return Err("protocol session has failed".to_string());
        }

        self.remote_node_id = Some(remote_node_id);
        self.state = ProtocolState::Established;

        Ok(())
    }

    pub fn mark_handshake_pending(&mut self) -> Result<(), String> {
        match self.state {
            ProtocolState::Connected => {
                self.state = ProtocolState::HandshakePending;
                Ok(())
            }

            ProtocolState::HandshakePending => Err("handshake is already pending".to_string()),

            ProtocolState::Established => Err("session is already established".to_string()),

            ProtocolState::Closing => Err("session is closing".to_string()),

            ProtocolState::Closed => Err("session is closed".to_string()),

            ProtocolState::Failed => Err("session has failed".to_string()),
        }
    }

    pub fn record_sent(&mut self, message: &ProtocolMessage) -> Result<(), String> {
        if self.state == ProtocolState::Closed {
            return Err("cannot send on closed session".to_string());
        }

        if self.state == ProtocolState::Closing {
            return Err("cannot send while session is closing".to_string());
        }

        if self.state == ProtocolState::Failed {
            return Err("cannot send on failed session".to_string());
        }

        message.validate()?;

        self.sent_messages = self
            .sent_messages
            .checked_add(1)
            .ok_or_else(|| "sent message counter overflow".to_string())?;

        Ok(())
    }

    pub fn record_received(&mut self, message: &ProtocolMessage) -> Result<(), String> {
        if self.state == ProtocolState::Closed {
            return Err("cannot receive on closed session".to_string());
        }

        if self.state == ProtocolState::Failed {
            return Err("cannot receive on failed session".to_string());
        }

        message.validate()?;

        self.received_messages = self
            .received_messages
            .checked_add(1)
            .ok_or_else(|| "received message counter overflow".to_string())?;

        Ok(())
    }

    pub fn begin_close(&mut self) -> Result<(), String> {
        match self.state {
            ProtocolState::Connected
            | ProtocolState::HandshakePending
            | ProtocolState::Established => {
                self.state = ProtocolState::Closing;
                Ok(())
            }

            ProtocolState::Closing => Err("session is already closing".to_string()),

            ProtocolState::Closed => Err("session is already closed".to_string()),

            ProtocolState::Failed => Err("failed session cannot begin normal close".to_string()),
        }
    }

    pub fn close(&mut self) {
        self.state = ProtocolState::Closed;
    }

    pub fn fail(&mut self) {
        self.state = ProtocolState::Failed;
    }

    pub fn is_established(&self) -> bool {
        self.state == ProtocolState::Established
    }

    pub fn is_closed(&self) -> bool {
        self.state == ProtocolState::Closed
    }

    pub fn is_failed(&self) -> bool {
        self.state == ProtocolState::Failed
    }
}

pub fn validate_message_payload(message_type: MessageType, payload: &[u8]) -> Result<(), String> {
    if payload.len() > MAX_MESSAGE_SIZE {
        return Err("message payload exceeds maximum size".to_string());
    }

    match message_type {
        MessageType::Ping => {
            if !payload.is_empty() {
                return Err("ping payload must be empty".to_string());
            }
        }

        MessageType::Pong => {
            if !payload.is_empty() {
                return Err("pong payload must be empty".to_string());
            }
        }

        MessageType::Handshake => {
            if payload.is_empty() {
                return Err("handshake payload cannot be empty".to_string());
            }

            if payload.len() > MAX_USER_AGENT_LENGTH + 64 {
                return Err("handshake payload is too large".to_string());
            }
        }

        MessageType::GetPeers => {
            if !payload.is_empty() {
                return Err("get peers payload must be empty".to_string());
            }
        }

        MessageType::Peers => {
            if payload.len() > MAX_PEER_ADDRESSES * 32 {
                return Err("peers payload exceeds maximum size".to_string());
            }
        }

        MessageType::GetHeaders => {
            if payload.is_empty() {
                return Err("get headers payload cannot be empty".to_string());
            }

            if payload.len() > MAX_GET_HEADERS * 32 {
                return Err("get headers payload exceeds maximum size".to_string());
            }
        }

        MessageType::Headers => {
            if payload.is_empty() {
                return Err("headers payload cannot be empty".to_string());
            }

            if payload.len() > MAX_GET_HEADERS * 128 {
                return Err("headers payload exceeds maximum size".to_string());
            }
        }

        MessageType::GetBlocks => {
            if payload.is_empty() {
                return Err("get blocks payload cannot be empty".to_string());
            }

            if payload.len() > MAX_GET_BLOCKS * 32 {
                return Err("get blocks payload exceeds maximum size".to_string());
            }
        }

        MessageType::Blocks => {
            if payload.is_empty() {
                return Err("blocks payload cannot be empty".to_string());
            }

            if payload.len() > MAX_GET_BLOCKS * 2_048 {
                return Err("blocks payload exceeds maximum size".to_string());
            }
        }

        MessageType::Transaction => {
            if payload.is_empty() {
                return Err("transaction payload cannot be empty".to_string());
            }
        }

        MessageType::Block => {
            if payload.is_empty() {
                return Err("block payload cannot be empty".to_string());
            }
        }
    }

    Ok(())
}

pub fn validate_peer_id(peer_id: &PeerId) -> Result<(), String> {
    if *peer_id == [0u8; 32] {
        return Err("peer id cannot be zero".to_string());
    }

    Ok(())
}

pub fn validate_peer_pair(local: &PeerId, remote: &PeerId) -> Result<(), String> {
    validate_peer_id(local)?;
    validate_peer_id(remote)?;

    if local == remote {
        return Err("local and remote peer ids cannot be identical".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer_id(value: u8) -> PeerId {
        [value; 32]
    }

    #[test]
    fn protocol_version_is_one() {
        assert_eq!(P2P_PROTOCOL_VERSION, 1);
    }

    #[test]
    fn ping_message_can_be_created() {
        let message = ProtocolMessage::empty(MessageType::Ping).expect("ping");

        assert_eq!(message.version, P2P_PROTOCOL_VERSION);
        assert_eq!(message.message_type, MessageType::Ping);
        assert!(message.is_empty());
        assert!(message.validate().is_ok());
    }

    #[test]
    fn pong_message_can_be_created() {
        let message = ProtocolMessage::empty(MessageType::Pong).expect("pong");

        assert!(message.validate().is_ok());
    }

    #[test]
    fn ping_with_payload_is_rejected() {
        let message =
            ProtocolMessage::new(MessageType::Ping, vec![1]).expect("message construction");

        assert!(message.validate().is_err());
    }

    #[test]
    fn pong_with_payload_is_rejected() {
        let message =
            ProtocolMessage::new(MessageType::Pong, vec![1]).expect("message construction");

        assert!(message.validate().is_err());
    }

    #[test]
    fn oversized_message_is_rejected() {
        let payload = vec![0u8; MAX_MESSAGE_SIZE + 1];

        assert!(ProtocolMessage::new(MessageType::Transaction, payload).is_err());
    }

    #[test]
    fn unknown_message_type_is_rejected() {
        assert_eq!(MessageType::from_u8(255), None);
    }

    #[test]
    fn known_message_types_are_decoded() {
        assert_eq!(MessageType::from_u8(1), Some(MessageType::Ping));

        assert_eq!(MessageType::from_u8(11), Some(MessageType::Block));
    }

    #[test]
    fn request_and_response_categories_are_correct() {
        assert!(MessageType::Ping.is_request());
        assert!(MessageType::GetPeers.is_request());

        assert!(MessageType::Pong.is_response());
        assert!(MessageType::Peers.is_response());
    }

    #[test]
    fn protocol_header_is_created() {
        let header =
            ProtocolHeader::new(P2P_PROTOCOL_VERSION, MessageType::Ping, 0).expect("header");

        assert!(header.validate().is_ok());
        assert_eq!(header.payload_length, 0);
    }

    #[test]
    fn invalid_header_version_is_rejected() {
        let header = ProtocolHeader::new(P2P_PROTOCOL_VERSION + 1, MessageType::Ping, 0);

        assert!(header.is_err());
    }

    #[test]
    fn local_zero_peer_id_is_rejected() {
        assert!(ProtocolSession::new([0u8; 32]).is_err());
    }

    #[test]
    fn protocol_session_starts_connected() {
        let session = ProtocolSession::new(peer_id(1)).expect("session");

        assert_eq!(session.state(), ProtocolState::Connected);

        assert!(!session.is_established());
        assert!(!session.is_closed());
        assert!(!session.is_failed());
    }

    #[test]
    fn handshake_pending_state_works() {
        let mut session = ProtocolSession::new(peer_id(1)).expect("session");

        session.mark_handshake_pending().expect("handshake pending");

        assert_eq!(session.state(), ProtocolState::HandshakePending);
    }

    #[test]
    fn remote_peer_establishes_session() {
        let mut session = ProtocolSession::new(peer_id(1)).expect("session");

        session.set_remote_node_id(peer_id(2)).expect("remote peer");

        assert_eq!(session.remote_node_id(), Some(peer_id(2)));

        assert!(session.is_established());
    }

    #[test]
    fn zero_remote_peer_is_rejected() {
        let mut session = ProtocolSession::new(peer_id(1)).expect("session");

        assert!(session.set_remote_node_id([0u8; 32]).is_err());
    }

    #[test]
    fn self_connection_is_rejected() {
        let mut session = ProtocolSession::new(peer_id(1)).expect("session");

        assert!(session.set_remote_node_id(peer_id(1)).is_err());

        assert!(session.is_failed());
    }

    #[test]
    fn sent_message_counter_increases() {
        let mut session = ProtocolSession::new(peer_id(1)).expect("session");

        let message = ProtocolMessage::empty(MessageType::Ping).expect("ping");

        session.record_sent(&message).expect("send");

        assert_eq!(session.sent_messages(), 1);
    }

    #[test]
    fn received_message_counter_increases() {
        let mut session = ProtocolSession::new(peer_id(1)).expect("session");

        let message = ProtocolMessage::empty(MessageType::Ping).expect("ping");

        session.record_received(&message).expect("receive");

        assert_eq!(session.received_messages(), 1);
    }

    #[test]
    fn close_state_works() {
        let mut session = ProtocolSession::new(peer_id(1)).expect("session");

        session.begin_close().expect("begin close");

        assert_eq!(session.state(), ProtocolState::Closing);

        session.close();

        assert!(session.is_closed());
    }

    #[test]
    fn failed_session_cannot_send() {
        let mut session = ProtocolSession::new(peer_id(1)).expect("session");

        session.fail();

        let message = ProtocolMessage::empty(MessageType::Ping).expect("ping");

        assert!(session.record_sent(&message).is_err());
    }

    #[test]
    fn peer_id_validation_works() {
        assert!(validate_peer_id(&peer_id(1)).is_ok());

        assert!(validate_peer_id(&[0u8; 32]).is_err());
    }

    #[test]
    fn peer_pair_validation_works() {
        assert!(validate_peer_pair(&peer_id(1), &peer_id(2)).is_ok());

        assert!(validate_peer_pair(&peer_id(1), &peer_id(1)).is_err());
    }

    #[test]
    fn handshake_payload_must_not_be_empty() {
        let message = ProtocolMessage::empty(MessageType::Handshake).expect("message");

        assert!(message.validate().is_err());
    }

    #[test]
    fn transaction_payload_must_not_be_empty() {
        let message = ProtocolMessage::empty(MessageType::Transaction).expect("message");

        assert!(message.validate().is_err());
    }

    #[test]
    fn block_payload_must_not_be_empty() {
        let message = ProtocolMessage::empty(MessageType::Block).expect("message");

        assert!(message.validate().is_err());
    }

    #[test]
    fn get_peers_payload_must_be_empty() {
        let message = ProtocolMessage::empty(MessageType::GetPeers).expect("message");

        assert!(message.validate().is_ok());
    }

    #[test]
    fn get_headers_requires_payload() {
        let message = ProtocolMessage::empty(MessageType::GetHeaders).expect("message");

        assert!(message.validate().is_err());
    }

    #[test]
    fn get_blocks_requires_payload() {
        let message = ProtocolMessage::empty(MessageType::GetBlocks).expect("message");

        assert!(message.validate().is_err());
    }
}
