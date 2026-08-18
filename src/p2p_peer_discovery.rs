use crate::p2p::PeerId;
use crate::p2p_manager::{ManagedPeer, PeerManager};
use crate::p2p_protocol::{MessageType, ProtocolMessage, MAX_PEER_ADDRESSES};

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

const COUNT_SIZE: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPeer {
    pub peer_id: PeerId,
    pub address: SocketAddr,
}

impl DiscoveredPeer {
    pub fn new(peer_id: PeerId, address: SocketAddr) -> Result<Self, String> {
        if peer_id == [0u8; 32] {
            return Err("peer id cannot be zero".to_string());
        }

        Ok(Self { peer_id, address })
    }
}

#[derive(Debug, Default)]
pub struct PeerDiscovery {
    discovered: HashSet<PeerId>,
}

impl PeerDiscovery {
    pub fn new() -> Self {
        Self {
            discovered: HashSet::new(),
        }
    }

    pub fn discovered_count(&self) -> usize {
        self.discovered.len()
    }

    pub fn contains(&self, peer_id: &PeerId) -> bool {
        self.discovered.contains(peer_id)
    }

    pub fn discover(&mut self, peer: &DiscoveredPeer) -> Result<bool, String> {
        if peer.peer_id == [0u8; 32] {
            return Err("peer id cannot be zero".to_string());
        }

        Ok(self.discovered.insert(peer.peer_id))
    }

    pub fn clear(&mut self) {
        self.discovered.clear();
    }
}

pub fn add_discovered_peer(
    manager: &mut PeerManager,
    peer: &DiscoveredPeer,
) -> Result<bool, String> {
    if peer.peer_id == [0u8; 32] {
        return Err("peer id cannot be zero".to_string());
    }

    if manager.contains_peer(&peer.peer_id) {
        return Ok(false);
    }

    let managed_peer = ManagedPeer::new(peer.peer_id, peer.address)?;

    manager.add_peer(managed_peer)?;

    Ok(true)
}

pub fn discover_and_add_peer(
    discovery: &mut PeerDiscovery,
    manager: &mut PeerManager,
    peer: DiscoveredPeer,
) -> Result<bool, String> {
    if peer.peer_id == [0u8; 32] {
        return Err("peer id cannot be zero".to_string());
    }

    let discovered = discovery.discover(&peer)?;

    if !discovered {
        return Ok(false);
    }

    add_discovered_peer(manager, &peer)
}

pub fn add_peers(manager: &mut PeerManager, peers: &[DiscoveredPeer]) -> Result<usize, String> {
    if peers.len() > MAX_PEER_ADDRESSES {
        return Err("too many peers".to_string());
    }

    let mut added = 0usize;

    for peer in peers {
        if peer.peer_id == [0u8; 32] {
            return Err("peer id cannot be zero".to_string());
        }

        if manager.contains_peer(&peer.peer_id) {
            continue;
        }

        let managed_peer = ManagedPeer::new(peer.peer_id, peer.address)?;

        manager.add_peer(managed_peer)?;

        added += 1;
    }

    Ok(added)
}

pub fn connected_discovered_peers(manager: &PeerManager) -> Vec<DiscoveredPeer> {
    manager
        .connected_peers()
        .map(|peer| DiscoveredPeer {
            peer_id: peer.peer_id,
            address: peer.address,
        })
        .collect()
}

pub fn all_discovered_peers(manager: &PeerManager) -> Vec<DiscoveredPeer> {
    manager
        .peers()
        .map(|peer| DiscoveredPeer {
            peer_id: peer.peer_id,
            address: peer.address,
        })
        .collect()
}

/*
 * ---------------------------------------------------------
 * GETPEERS / PEERS PAYLOAD
 * ---------------------------------------------------------
 *
 * Peers payload format:
 *
 * [2 bytes] peer count
 *
 * For every peer:
 *
 * [32 bytes] PeerId
 * [1 byte ] address family
 *           4 = IPv4
 *           6 = IPv6
 * [4/16 bytes] IP address
 * [2 bytes] port
 *
 * All integers are big-endian.
 */

pub fn encode_peers_payload(peers: &[DiscoveredPeer]) -> Result<Vec<u8>, String> {
    if peers.len() > MAX_PEER_ADDRESSES {
        return Err("too many peers".to_string());
    }

    let mut buffer = Vec::new();

    let count = u16::try_from(peers.len()).map_err(|_| "peer count exceeds u16".to_string())?;

    buffer.extend_from_slice(&count.to_be_bytes());

    let mut seen = HashSet::<PeerId>::new();

    for peer in peers {
        if peer.peer_id == [0u8; 32] {
            return Err("peer id cannot be zero".to_string());
        }

        if !seen.insert(peer.peer_id) {
            return Err("duplicate peer id in payload".to_string());
        }

        buffer.extend_from_slice(&peer.peer_id);

        match peer.address.ip() {
            IpAddr::V4(ip) => {
                buffer.push(4);

                buffer.extend_from_slice(&ip.octets());
            }

            IpAddr::V6(ip) => {
                buffer.push(6);

                buffer.extend_from_slice(&ip.octets());
            }
        }

        buffer.extend_from_slice(&peer.address.port().to_be_bytes());
    }

    Ok(buffer)
}

pub fn decode_peers_payload(payload: &[u8]) -> Result<Vec<DiscoveredPeer>, String> {
    if payload.len() < COUNT_SIZE {
        return Err("peers payload is too short".to_string());
    }

    let count = u16::from_be_bytes([payload[0], payload[1]]) as usize;

    if count > MAX_PEER_ADDRESSES {
        return Err("peer count exceeds maximum".to_string());
    }

    if count == 0 {
        if payload.len() != COUNT_SIZE {
            return Err("invalid empty peers payload".to_string());
        }

        return Ok(Vec::new());
    }

    let mut cursor = COUNT_SIZE;

    let mut result = Vec::with_capacity(count);

    let mut seen = HashSet::<PeerId>::new();

    for _ in 0..count {
        if payload.len().saturating_sub(cursor) < 33 {
            return Err("truncated peer entry".to_string());
        }

        let mut peer_id = [0u8; 32];

        peer_id.copy_from_slice(&payload[cursor..cursor + 32]);

        cursor += 32;

        if peer_id == [0u8; 32] {
            return Err("peer id cannot be zero".to_string());
        }

        if !seen.insert(peer_id) {
            return Err("duplicate peer id".to_string());
        }

        let family = payload[cursor];

        cursor += 1;

        let address = match family {
            4 => {
                if payload.len().saturating_sub(cursor) < 6 {
                    return Err("truncated ipv4 peer entry".to_string());
                }

                let ip = Ipv4Addr::new(
                    payload[cursor],
                    payload[cursor + 1],
                    payload[cursor + 2],
                    payload[cursor + 3],
                );

                cursor += 4;

                let port = u16::from_be_bytes([payload[cursor], payload[cursor + 1]]);

                cursor += 2;

                SocketAddr::new(IpAddr::V4(ip), port)
            }

            6 => {
                if payload.len().saturating_sub(cursor) < 18 {
                    return Err("truncated ipv6 peer entry".to_string());
                }

                let mut octets = [0u8; 16];

                octets.copy_from_slice(&payload[cursor..cursor + 16]);

                cursor += 16;

                let port = u16::from_be_bytes([payload[cursor], payload[cursor + 1]]);

                cursor += 2;

                SocketAddr::new(IpAddr::V6(Ipv6Addr::from(octets)), port)
            }

            _ => {
                return Err("unknown address family".to_string());
            }
        };

        result.push(DiscoveredPeer { peer_id, address });
    }

    if cursor != payload.len() {
        return Err("unexpected trailing data in peers payload".to_string());
    }

    Ok(result)
}

pub fn build_get_peers_message() -> Result<ProtocolMessage, String> {
    ProtocolMessage::empty(MessageType::GetPeers)
}

pub fn build_peers_message(peers: &[DiscoveredPeer]) -> Result<ProtocolMessage, String> {
    let payload = encode_peers_payload(peers)?;

    ProtocolMessage::new(MessageType::Peers, payload)
}

pub fn decode_peers_message(message: &ProtocolMessage) -> Result<Vec<DiscoveredPeer>, String> {
    if message.message_type != MessageType::Peers {
        return Err("message is not a peers message".to_string());
    }

    decode_peers_payload(&message.payload)
}

pub fn apply_peers_message(
    local_peer_id: &PeerId,
    manager: &mut PeerManager,
    discovery: &mut PeerDiscovery,
    message: &ProtocolMessage,
) -> Result<usize, String> {
    if message.message_type != MessageType::Peers {
        return Err("message is not a peers message".to_string());
    }

    if *local_peer_id == [0u8; 32] {
        return Err("local peer id cannot be zero".to_string());
    }

    let peers = decode_peers_message(message)?;

    let mut accepted = Vec::with_capacity(peers.len());

    for peer in peers {
        if peer.peer_id == *local_peer_id {
            return Err("peer payload contains local node".to_string());
        }

        accepted.push(peer);
    }

    let mut added = 0usize;

    for peer in accepted {
        if !discovery.discover(&peer)? {
            continue;
        }

        if add_discovered_peer(manager, &peer)? {
            added += 1;
        }
    }

    Ok(added)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PEER_ENTRY_SIZE_IPV4: usize = 32 + 1 + 4 + 2;

    const PEER_ENTRY_SIZE_IPV6: usize = 32 + 1 + 16 + 2;

    fn peer_id(value: u8) -> PeerId {
        [value; 32]
    }

    /*
     * Generates a deterministic non-zero PeerId
     * from a usize.
     *
     * This avoids u8 overflow in tests when
     * MAX_PEER_ADDRESSES is larger than 255.
     */
    fn indexed_peer_id(index: usize) -> PeerId {
        let mut id = [0u8; 32];

        let bytes = (index as u64).to_be_bytes();

        id[..8].copy_from_slice(&bytes);

        /*
         * Make sure the ID can never become
         * the all-zero PeerId.
         */
        if id == [0u8; 32] {
            id[31] = 1;
        }

        id
    }

    fn address(port: u16) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], port))
    }

    fn ipv6_address(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port)
    }

    fn discovered_peer(value: u8, port: u16) -> DiscoveredPeer {
        DiscoveredPeer::new(peer_id(value), address(port)).expect("discovered peer")
    }

    #[test]
    fn discovered_peer_can_be_created() {
        let peer = discovered_peer(1, 8333);

        assert_eq!(peer.peer_id, peer_id(1));

        assert_eq!(peer.address, address(8333));
    }

    #[test]
    fn zero_peer_id_is_rejected() {
        let result = DiscoveredPeer::new([0u8; 32], address(8333));

        assert!(result.is_err());
    }

    #[test]
    fn discovery_starts_empty() {
        let discovery = PeerDiscovery::new();

        assert_eq!(discovery.discovered_count(), 0);
    }

    #[test]
    fn peer_can_be_discovered() {
        let mut discovery = PeerDiscovery::new();

        let peer = discovered_peer(1, 8333);

        let inserted = discovery.discover(&peer).expect("discover");

        assert!(inserted);

        assert!(discovery.contains(&peer_id(1)));

        assert_eq!(discovery.discovered_count(), 1);
    }

    #[test]
    fn duplicate_discovery_is_ignored() {
        let mut discovery = PeerDiscovery::new();

        let peer = discovered_peer(1, 8333);

        assert!(discovery.discover(&peer).expect("first discovery"));

        assert!(!discovery.discover(&peer).expect("second discovery"));

        assert_eq!(discovery.discovered_count(), 1);
    }

    #[test]
    fn discovery_can_be_cleared() {
        let mut discovery = PeerDiscovery::new();

        let peer = discovered_peer(1, 8333);

        discovery.discover(&peer).expect("discover");

        discovery.clear();

        assert_eq!(discovery.discovered_count(), 0);

        assert!(!discovery.contains(&peer_id(1)));
    }

    #[test]
    fn discovered_peer_can_be_added_to_manager() {
        let mut manager = PeerManager::new();

        let peer = discovered_peer(1, 8333);

        let added = add_discovered_peer(&mut manager, &peer).expect("add");

        assert!(added);

        assert_eq!(manager.peer_count(), 1);

        assert!(manager.contains_peer(&peer_id(1)));
    }

    #[test]
    fn existing_peer_is_not_added_twice() {
        let mut manager = PeerManager::new();

        let peer = discovered_peer(1, 8333);

        assert!(add_discovered_peer(&mut manager, &peer,).expect("first add"));

        assert!(!add_discovered_peer(&mut manager, &peer,).expect("second add"));

        assert_eq!(manager.peer_count(), 1);
    }

    #[test]
    fn discover_and_add_works() {
        let mut discovery = PeerDiscovery::new();

        let mut manager = PeerManager::new();

        let peer = discovered_peer(1, 8333);

        let added =
            discover_and_add_peer(&mut discovery, &mut manager, peer).expect("discover and add");

        assert!(added);

        assert_eq!(discovery.discovered_count(), 1);

        assert_eq!(manager.peer_count(), 1);
    }

    #[test]
    fn duplicate_discover_and_add_is_ignored() {
        let mut discovery = PeerDiscovery::new();

        let mut manager = PeerManager::new();

        let peer = discovered_peer(1, 8333);

        assert!(discover_and_add_peer(&mut discovery, &mut manager, peer.clone(),).expect("first"));

        assert!(!discover_and_add_peer(&mut discovery, &mut manager, peer,).expect("second"));

        assert_eq!(discovery.discovered_count(), 1);

        assert_eq!(manager.peer_count(), 1);
    }

    #[test]
    fn multiple_peers_can_be_added() {
        let mut manager = PeerManager::new();

        let peers = vec![
            discovered_peer(1, 8333),
            discovered_peer(2, 8334),
            discovered_peer(3, 8335),
        ];

        let added = add_peers(&mut manager, &peers).expect("add peers");

        assert_eq!(added, 3);

        assert_eq!(manager.peer_count(), 3);
    }

    #[test]
    fn existing_peers_are_skipped() {
        let mut manager = PeerManager::new();

        let peer1 = discovered_peer(1, 8333);

        let peer2 = discovered_peer(2, 8334);

        add_discovered_peer(&mut manager, &peer1).expect("first");

        let peers = vec![peer1, peer2];

        let added = add_peers(&mut manager, &peers).expect("add peers");

        assert_eq!(added, 1);

        assert_eq!(manager.peer_count(), 2);
    }

    #[test]
    fn connected_peers_are_returned() {
        let mut manager = PeerManager::new();

        let peer1 = discovered_peer(1, 8333);

        let peer2 = discovered_peer(2, 8334);

        add_discovered_peer(&mut manager, &peer1).expect("peer1");

        add_discovered_peer(&mut manager, &peer2).expect("peer2");

        manager.mark_connected(&peer_id(1)).expect("connect");

        let connected = connected_discovered_peers(&manager);

        assert_eq!(connected.len(), 1);

        assert_eq!(connected[0].peer_id, peer_id(1));

        assert_eq!(connected[0].address, address(8333));
    }

    #[test]
    fn all_peers_are_returned() {
        let mut manager = PeerManager::new();

        let peers = vec![discovered_peer(1, 8333), discovered_peer(2, 8334)];

        add_peers(&mut manager, &peers).expect("add");

        let all = all_discovered_peers(&manager);

        assert_eq!(all.len(), 2);

        assert!(all.iter().any(|peer| { peer.peer_id == peer_id(1) }));

        assert!(all.iter().any(|peer| { peer.peer_id == peer_id(2) }));
    }

    #[test]
    fn ipv4_peers_payload_roundtrip() {
        let peers = vec![discovered_peer(1, 8333), discovered_peer(2, 8334)];

        let encoded = encode_peers_payload(&peers).expect("encode");

        let decoded = decode_peers_payload(&encoded).expect("decode");

        assert_eq!(decoded, peers);
    }

    #[test]
    fn ipv6_peers_payload_roundtrip() {
        let peer = DiscoveredPeer::new(peer_id(1), ipv6_address(9333)).expect("peer");

        let peers = vec![peer];

        let encoded = encode_peers_payload(&peers).expect("encode");

        let decoded = decode_peers_payload(&encoded).expect("decode");

        assert_eq!(decoded, peers);
    }

    #[test]
    fn empty_peers_payload_is_valid() {
        let encoded = encode_peers_payload(&[]).expect("encode");

        let decoded = decode_peers_payload(&encoded).expect("decode");

        assert!(decoded.is_empty());
    }

    #[test]
    fn duplicate_peer_ids_are_rejected() {
        let peer = discovered_peer(1, 8333);

        assert!(encode_peers_payload(&[peer.clone(), peer]).is_err());
    }

    #[test]
    fn zero_peer_id_in_payload_is_rejected() {
        let mut payload = Vec::new();

        payload.extend_from_slice(&1u16.to_be_bytes());

        payload.extend_from_slice(&[0u8; 32]);

        payload.push(4);

        payload.extend_from_slice(&[127, 0, 0, 1]);

        payload.extend_from_slice(&8333u16.to_be_bytes());

        assert!(decode_peers_payload(&payload,).is_err());
    }

    #[test]
    fn invalid_address_family_is_rejected() {
        let mut payload = Vec::new();

        payload.extend_from_slice(&1u16.to_be_bytes());

        payload.extend_from_slice(&peer_id(1));

        payload.push(99);

        payload.extend_from_slice(&8333u16.to_be_bytes());

        assert!(decode_peers_payload(&payload,).is_err());
    }

    #[test]
    fn trailing_payload_data_is_rejected() {
        let peers = vec![discovered_peer(1, 8333)];

        let mut encoded = encode_peers_payload(&peers).expect("encode");

        encoded.push(0xFF);

        assert!(decode_peers_payload(&encoded,).is_err());
    }

    #[test]
    fn get_peers_message_is_created() {
        let message = build_get_peers_message().expect("get peers");

        assert_eq!(message.message_type, MessageType::GetPeers);

        assert!(message.payload.is_empty());

        assert!(message.validate().is_ok());
    }

    #[test]
    fn peers_message_is_created() {
        let peers = vec![discovered_peer(1, 8333), discovered_peer(2, 8334)];

        let message = build_peers_message(&peers).expect("peers message");

        assert_eq!(message.message_type, MessageType::Peers);

        let decoded = decode_peers_message(&message).expect("decode message");

        assert_eq!(decoded, peers);
    }

    #[test]
    fn wrong_message_type_is_rejected() {
        let message = ProtocolMessage::empty(MessageType::GetPeers).expect("message");

        assert!(decode_peers_message(&message,).is_err());
    }

    #[test]
    fn local_peer_is_rejected() {
        let local = peer_id(1);

        let peers = vec![discovered_peer(1, 8333), discovered_peer(2, 8334)];

        let message = build_peers_message(&peers).expect("message");

        let mut manager = PeerManager::new();

        let mut discovery = PeerDiscovery::new();

        assert!(apply_peers_message(&local, &mut manager, &mut discovery, &message,).is_err());

        assert_eq!(manager.peer_count(), 0);
    }

    #[test]
    fn peers_message_can_be_applied() {
        let local = peer_id(1);

        let peers = vec![discovered_peer(2, 8334), discovered_peer(3, 8335)];

        let message = build_peers_message(&peers).expect("message");

        let mut manager = PeerManager::new();

        let mut discovery = PeerDiscovery::new();

        let added =
            apply_peers_message(&local, &mut manager, &mut discovery, &message).expect("apply");

        assert_eq!(added, 2);

        assert_eq!(manager.peer_count(), 2);

        assert_eq!(discovery.discovered_count(), 2);
    }

    #[test]
    fn too_many_peers_are_rejected() {
        let peers = (0..=MAX_PEER_ADDRESSES)
            .map(|index| {
                let id = indexed_peer_id(index + 1);

                let port = 8333u16.saturating_add(index as u16);

                DiscoveredPeer::new(id, address(port)).expect("discovered peer")
            })
            .collect::<Vec<_>>();

        assert!(peers.len() > MAX_PEER_ADDRESSES);

        assert!(encode_peers_payload(&peers,).is_err());
    }

    #[test]
    fn peer_payload_size_is_reasonable() {
        let peers = vec![discovered_peer(1, 8333)];

        let payload = encode_peers_payload(&peers).expect("encode");

        assert!(payload.len() <= PEER_ENTRY_SIZE_IPV4 + COUNT_SIZE);

        const {
            assert!(PEER_ENTRY_SIZE_IPV6 > PEER_ENTRY_SIZE_IPV4);
        }
    }

    #[test]
    fn zero_local_peer_id_is_rejected() {
        let peers = vec![discovered_peer(1, 8333)];

        let message = build_peers_message(&peers).expect("message");

        let mut manager = PeerManager::new();

        let mut discovery = PeerDiscovery::new();

        assert!(apply_peers_message(&[0u8; 32], &mut manager, &mut discovery, &message,).is_err());

        assert_eq!(manager.peer_count(), 0);

        assert_eq!(discovery.discovered_count(), 0);
    }
}
