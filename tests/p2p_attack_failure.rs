use nio_blockchain::p2p_protocol::{
    validate_peer_id, validate_peer_pair, MessageType, ProtocolMessage, ProtocolSession,
    ProtocolState, MAX_MESSAGE_SIZE, P2P_PROTOCOL_VERSION,
};

use nio_blockchain::p2p_message_codec::{decode_frame, encode_frame, MessageFrame};

fn peer(value: u8) -> [u8; 32] {
    [value; 32]
}

#[test]
fn zero_peer_id_is_rejected() {
    assert!(validate_peer_id(&[0u8; 32]).is_err());
}

#[test]
fn valid_peer_id_is_accepted() {
    assert!(validate_peer_id(&peer(1)).is_ok());
}

#[test]
fn identical_peer_ids_are_rejected() {
    assert!(validate_peer_pair(&peer(1), &peer(1)).is_err());
}

#[test]
fn different_peer_ids_are_accepted() {
    assert!(validate_peer_pair(&peer(1), &peer(2)).is_ok());
}

#[test]
fn self_connection_moves_session_to_failed() {
    let mut session = ProtocolSession::new(peer(1)).expect("session");

    let result = session.set_remote_node_id(peer(1));

    assert!(result.is_err());
    assert_eq!(session.state(), ProtocolState::Failed);
}

#[test]
fn failed_session_cannot_send_messages() {
    let mut session = ProtocolSession::new(peer(1)).expect("session");

    session.fail();

    let message = ProtocolMessage::empty(MessageType::Ping).expect("ping");

    assert!(session.record_sent(&message).is_err());
}

#[test]
fn closed_session_cannot_send_messages() {
    let mut session = ProtocolSession::new(peer(1)).expect("session");

    session.begin_close().expect("begin close");
    session.close();

    let message = ProtocolMessage::empty(MessageType::Ping).expect("ping");

    assert!(session.record_sent(&message).is_err());
}

#[test]
fn oversized_protocol_message_is_rejected() {
    let payload = vec![0u8; MAX_MESSAGE_SIZE + 1];

    assert!(ProtocolMessage::new(MessageType::Transaction, payload).is_err());
}

#[test]
fn oversized_frame_is_rejected() {
    let payload = vec![0u8; MAX_MESSAGE_SIZE + 1];

    let frame = MessageFrame {
        version: P2P_PROTOCOL_VERSION,
        message_type: MessageType::Transaction,
        payload,
    };

    assert!(encode_frame(&frame).is_err());
}

#[test]
fn invalid_magic_is_rejected() {
    let frame = MessageFrame::new(MessageType::Ping, Vec::new()).expect("frame");

    let mut encoded = encode_frame(&frame).expect("encode");
    encoded[0] ^= 0xff;

    assert!(decode_frame(&encoded).is_err());
}

#[test]
fn invalid_version_is_rejected() {
    let frame = MessageFrame::new(MessageType::Ping, Vec::new()).expect("frame");

    let mut encoded = encode_frame(&frame).expect("encode");
    encoded[4..8].copy_from_slice(&(P2P_PROTOCOL_VERSION + 1).to_be_bytes());

    assert!(decode_frame(&encoded).is_err());
}

#[test]
fn unknown_message_type_is_rejected() {
    let frame = MessageFrame::new(MessageType::Ping, Vec::new()).expect("frame");

    let mut encoded = encode_frame(&frame).expect("encode");
    encoded[8] = 0xff;

    assert!(decode_frame(&encoded).is_err());
}

#[test]
fn checksum_tampering_is_rejected() {
    let frame = MessageFrame::new(MessageType::Transaction, vec![1, 2, 3, 4]).expect("frame");

    let mut encoded = encode_frame(&frame).expect("encode");
    let last = encoded.len() - 1;
    encoded[last] ^= 0xff;

    assert!(decode_frame(&encoded).is_err());
}

#[test]
fn payload_tampering_is_rejected() {
    let frame = MessageFrame::new(MessageType::Transaction, vec![1, 2, 3, 4]).expect("frame");

    let mut encoded = encode_frame(&frame).expect("encode");
    let payload_start = 13;
    encoded[payload_start] ^= 0xff;

    assert!(decode_frame(&encoded).is_err());
}

#[test]
fn truncated_frame_is_rejected() {
    let frame = MessageFrame::new(MessageType::Ping, Vec::new()).expect("frame");

    let encoded = encode_frame(&frame).expect("encode");
    let truncated = &encoded[..encoded.len() - 1];

    assert!(decode_frame(truncated).is_err());
}

#[test]
fn fake_payload_length_is_rejected() {
    let frame = MessageFrame::new(MessageType::Transaction, vec![1, 2, 3, 4]).expect("frame");

    let mut encoded = encode_frame(&frame).expect("encode");
    encoded[9..13].copy_from_slice(&u32::MAX.to_be_bytes());

    assert!(decode_frame(&encoded).is_err());
}

#[test]
fn empty_transaction_is_rejected() {
    let result = ProtocolMessage::empty(MessageType::Transaction).expect("message");

    assert!(result.validate().is_err());
}

#[test]
fn empty_block_is_rejected() {
    let result = ProtocolMessage::empty(MessageType::Block).expect("message");

    assert!(result.validate().is_err());
}

#[test]
fn ping_payload_is_rejected() {
    let message = ProtocolMessage::new(MessageType::Ping, vec![1]).expect("message");

    assert!(message.validate().is_err());
}

#[test]
fn pong_payload_is_rejected() {
    let message = ProtocolMessage::new(MessageType::Pong, vec![1]).expect("message");

    assert!(message.validate().is_err());
}

#[test]
fn get_peers_payload_is_rejected() {
    let message = ProtocolMessage::new(MessageType::GetPeers, vec![1]).expect("message");

    assert!(message.validate().is_err());
}

#[test]
fn empty_get_headers_is_rejected() {
    let message = ProtocolMessage::empty(MessageType::GetHeaders).expect("message");

    assert!(message.validate().is_err());
}

#[test]
fn empty_get_blocks_is_rejected() {
    let message = ProtocolMessage::empty(MessageType::GetBlocks).expect("message");

    assert!(message.validate().is_err());
}

#[test]
fn protocol_session_cannot_use_zero_local_id() {
    assert!(ProtocolSession::new([0u8; 32]).is_err());
}

#[test]
fn protocol_session_starts_connected() {
    let session = ProtocolSession::new(peer(1)).expect("session");

    assert_eq!(session.state(), ProtocolState::Connected);
}

#[test]
fn handshake_pending_cannot_be_entered_twice() {
    let mut session = ProtocolSession::new(peer(1)).expect("session");

    session.mark_handshake_pending().expect("first transition");

    assert!(session.mark_handshake_pending().is_err());
}

#[test]
fn established_session_cannot_return_to_handshake_pending() {
    let mut session = ProtocolSession::new(peer(1)).expect("session");

    session.set_remote_node_id(peer(2)).expect("establish");

    assert_eq!(session.state(), ProtocolState::Established);
    assert!(session.mark_handshake_pending().is_err());
}

#[test]
fn closing_session_cannot_send() {
    let mut session = ProtocolSession::new(peer(1)).expect("session");

    session.begin_close().expect("close");

    let message = ProtocolMessage::empty(MessageType::Ping).expect("ping");

    assert!(session.record_sent(&message).is_err());
}
