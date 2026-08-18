use nio_blockchain::p2p::PeerId;
use nio_blockchain::p2p_manager::PeerManager;
use nio_blockchain::p2p_peer_discovery::{
    add_peers, apply_peers_message, build_peers_message, decode_peers_payload,
    encode_peers_payload, DiscoveredPeer, PeerDiscovery,
};
use nio_blockchain::p2p_protocol::MAX_PEER_ADDRESSES;

use std::net::{IpAddr, Ipv6Addr, SocketAddr};

fn peer_id(value: u8) -> PeerId {
    [value; 32]
}

fn unique_peer_id(value: usize) -> PeerId {
    let mut id = [0u8; 32];

    let value = value as u64;

    id[0..8].copy_from_slice(&value.to_be_bytes());

    id[31] = 1;

    id
}

fn address(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

fn ipv6_address(port: u16) -> SocketAddr {
    SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port)
}

fn discovered_peer(value: u8, port: u16) -> DiscoveredPeer {
    DiscoveredPeer::new(peer_id(value), address(port)).expect("valid discovered peer")
}

fn unique_discovered_peer(value: usize) -> DiscoveredPeer {
    DiscoveredPeer::new(unique_peer_id(value), address(8333 + (value % 1000) as u16))
        .expect("valid unique discovered peer")
}

#[test]
fn rejects_zero_peer_id() {
    let result = DiscoveredPeer::new([0u8; 32], address(8333));

    assert!(result.is_err());
}

#[test]
fn rejects_too_many_peers_during_encoding() {
    let peers = (1..=MAX_PEER_ADDRESSES + 1)
        .map(unique_discovered_peer)
        .collect::<Vec<_>>();

    assert_eq!(peers.len(), MAX_PEER_ADDRESSES + 1);

    assert!(encode_peers_payload(&peers).is_err());
}

#[test]
fn rejects_too_many_peers_during_manager_add() {
    let peers = (1..=MAX_PEER_ADDRESSES + 1)
        .map(unique_discovered_peer)
        .collect::<Vec<_>>();

    assert_eq!(peers.len(), MAX_PEER_ADDRESSES + 1);

    let mut manager = PeerManager::new();

    assert!(add_peers(&mut manager, &peers).is_err());

    assert_eq!(manager.peer_count(), 0);
}

#[test]
fn rejects_duplicate_peer_ids_in_payload() {
    let peer = discovered_peer(1, 8333);

    let peers = vec![peer.clone(), peer];

    assert!(encode_peers_payload(&peers).is_err());
}

#[test]
fn rejects_zero_peer_id_in_raw_payload() {
    let mut payload = Vec::new();

    payload.extend_from_slice(&1u16.to_be_bytes());
    payload.extend_from_slice(&[0u8; 32]);
    payload.push(4);
    payload.extend_from_slice(&[127, 0, 0, 1]);
    payload.extend_from_slice(&8333u16.to_be_bytes());

    assert!(decode_peers_payload(&payload).is_err());
}

#[test]
fn rejects_truncated_ipv4_payload() {
    let mut payload = Vec::new();

    payload.extend_from_slice(&1u16.to_be_bytes());
    payload.extend_from_slice(&peer_id(1));
    payload.push(4);
    payload.extend_from_slice(&[127, 0]);

    assert!(decode_peers_payload(&payload).is_err());
}

#[test]
fn rejects_truncated_ipv6_payload() {
    let mut payload = Vec::new();

    payload.extend_from_slice(&1u16.to_be_bytes());
    payload.extend_from_slice(&peer_id(1));
    payload.push(6);
    payload.extend_from_slice(&[0u8; 8]);

    assert!(decode_peers_payload(&payload).is_err());
}

#[test]
fn rejects_invalid_address_family() {
    let mut payload = Vec::new();

    payload.extend_from_slice(&1u16.to_be_bytes());
    payload.extend_from_slice(&peer_id(1));
    payload.push(99);

    assert!(decode_peers_payload(&payload).is_err());
}

#[test]
fn rejects_trailing_bytes() {
    let peers = vec![discovered_peer(1, 8333)];

    let mut payload = encode_peers_payload(&peers).expect("encode");

    payload.extend_from_slice(&[0xAA, 0xBB, 0xCC]);

    assert!(decode_peers_payload(&payload).is_err());
}

#[test]
fn rejects_fake_peer_count() {
    let peers = vec![discovered_peer(1, 8333)];

    let mut payload = encode_peers_payload(&peers).expect("encode");

    payload[0] = 0;
    payload[1] = 2;

    assert!(decode_peers_payload(&payload).is_err());
}

#[test]
fn rejects_duplicate_peer_ids_from_raw_payload() {
    let peers = vec![discovered_peer(1, 8333), discovered_peer(2, 8334)];

    let mut payload = encode_peers_payload(&peers).expect("encode");

    let first_start = 2usize;

    let second_start = 2usize + 32 + 1 + 4 + 2;

    let first_id = {
        let mut copied = [0u8; 32];

        copied.copy_from_slice(&payload[first_start..first_start + 32]);

        copied
    };

    payload[second_start..second_start + 32].copy_from_slice(&first_id);

    assert!(decode_peers_payload(&payload).is_err());
}

#[test]
fn rejects_local_peer_without_modifying_state() {
    let local = peer_id(1);

    let peers = vec![discovered_peer(2, 8334), discovered_peer(1, 8333)];

    let message = build_peers_message(&peers).expect("build message");

    let mut manager = PeerManager::new();

    let mut discovery = PeerDiscovery::new();

    let result = apply_peers_message(&local, &mut manager, &mut discovery, &message);

    assert!(result.is_err());

    assert_eq!(manager.peer_count(), 0);

    assert_eq!(discovery.discovered_count(), 0);
}

#[test]
fn valid_peer_is_accepted() {
    let local = peer_id(1);

    let peers = vec![discovered_peer(2, 8334)];

    let message = build_peers_message(&peers).expect("build message");

    let mut manager = PeerManager::new();

    let mut discovery = PeerDiscovery::new();

    let added = apply_peers_message(&local, &mut manager, &mut discovery, &message).expect("apply");

    assert_eq!(added, 1);

    assert_eq!(manager.peer_count(), 1);

    assert_eq!(discovery.discovered_count(), 1);
}

#[test]
fn malicious_peer_after_valid_peer_does_not_partially_modify_state() {
    let local = peer_id(1);

    let peers = vec![discovered_peer(2, 8334), discovered_peer(1, 8333)];

    let message = build_peers_message(&peers).expect("build message");

    let mut manager = PeerManager::new();

    let mut discovery = PeerDiscovery::new();

    let result = apply_peers_message(&local, &mut manager, &mut discovery, &message);

    assert!(result.is_err());

    assert_eq!(manager.peer_count(), 0);

    assert_eq!(discovery.discovered_count(), 0);
}

#[test]
fn ipv6_peer_is_accepted() {
    let peer = DiscoveredPeer::new(peer_id(9), ipv6_address(9333)).expect("ipv6 peer");

    let payload = encode_peers_payload(std::slice::from_ref(&peer)).expect("encode");

    let decoded = decode_peers_payload(&payload).expect("decode");

    assert_eq!(decoded, vec![peer]);
}
