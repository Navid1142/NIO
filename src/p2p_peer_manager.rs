use crate::p2p::PeerId;
use crate::p2p_protocol::ProtocolState;
use std::collections::HashMap;
use std::net::SocketAddr;

pub const DEFAULT_MAX_PEERS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerInfo {
    peer_id: PeerId,
    address: SocketAddr,
    state: ProtocolState,
}

impl PeerInfo {
    pub fn new(peer_id: PeerId, address: SocketAddr) -> Result<Self, String> {
        if peer_id == [0u8; 32] {
            return Err("peer id cannot be zero".to_string());
        }

        Ok(Self {
            peer_id,
            address,
            state: ProtocolState::Connected,
        })
    }

    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn state(&self) -> ProtocolState {
        self.state
    }

    pub fn set_state(&mut self, state: ProtocolState) {
        self.state = state;
    }

    pub fn is_connected(&self) -> bool {
        matches!(
            self.state,
            ProtocolState::Connected | ProtocolState::HandshakePending | ProtocolState::Established
        )
    }
}

#[derive(Debug)]
pub struct PeerManager {
    local_peer_id: PeerId,
    max_peers: usize,
    peers: HashMap<PeerId, PeerInfo>,
}

impl PeerManager {
    pub fn new(local_peer_id: PeerId) -> Result<Self, String> {
        Self::with_max_peers(local_peer_id, DEFAULT_MAX_PEERS)
    }

    pub fn with_max_peers(local_peer_id: PeerId, max_peers: usize) -> Result<Self, String> {
        if local_peer_id == [0u8; 32] {
            return Err("local peer id cannot be zero".to_string());
        }

        if max_peers == 0 {
            return Err("maximum peer count cannot be zero".to_string());
        }

        Ok(Self {
            local_peer_id,
            max_peers,
            peers: HashMap::new(),
        })
    }

    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    pub fn max_peers(&self) -> usize {
        self.max_peers
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    pub fn is_full(&self) -> bool {
        self.peers.len() >= self.max_peers
    }

    pub fn contains(&self, peer_id: &PeerId) -> bool {
        self.peers.contains_key(peer_id)
    }

    pub fn get(&self, peer_id: &PeerId) -> Option<&PeerInfo> {
        self.peers.get(peer_id)
    }

    pub fn get_mut(&mut self, peer_id: &PeerId) -> Option<&mut PeerInfo> {
        self.peers.get_mut(peer_id)
    }

    pub fn add_peer(&mut self, peer_id: PeerId, address: SocketAddr) -> Result<(), String> {
        if peer_id == [0u8; 32] {
            return Err("peer id cannot be zero".to_string());
        }

        if peer_id == self.local_peer_id {
            return Err("cannot add local peer as remote peer".to_string());
        }

        if self.peers.contains_key(&peer_id) {
            return Err("peer already exists".to_string());
        }

        if self.is_full() {
            return Err("peer manager is full".to_string());
        }

        let peer = PeerInfo::new(peer_id, address)?;

        self.peers.insert(peer_id, peer);

        Ok(())
    }

    pub fn remove_peer(&mut self, peer_id: &PeerId) -> Option<PeerInfo> {
        self.peers.remove(peer_id)
    }

    pub fn set_peer_state(&mut self, peer_id: &PeerId, state: ProtocolState) -> Result<(), String> {
        let peer = self
            .peers
            .get_mut(peer_id)
            .ok_or_else(|| "peer not found".to_string())?;

        peer.set_state(state);

        Ok(())
    }

    pub fn connected_peers(&self) -> Vec<PeerId> {
        self.peers
            .values()
            .filter(|peer| peer.is_connected())
            .map(|peer| peer.peer_id())
            .collect()
    }

    pub fn peer_ids(&self) -> Vec<PeerId> {
        self.peers.keys().copied().collect()
    }

    pub fn clear(&mut self) {
        self.peers.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer_id(value: u8) -> PeerId {
        [value; 32]
    }

    fn address(port: u16) -> SocketAddr {
        format!("127.0.0.1:{}", port)
            .parse()
            .expect("valid socket address")
    }

    #[test]
    fn manager_rejects_zero_local_peer_id() {
        let result = PeerManager::new([0u8; 32]);

        assert!(result.is_err());
    }

    #[test]
    fn manager_rejects_zero_max_peers() {
        let result = PeerManager::with_max_peers(peer_id(1), 0);

        assert!(result.is_err());
    }

    #[test]
    fn manager_starts_empty() {
        let manager = PeerManager::new(peer_id(1)).expect("manager");

        assert_eq!(manager.peer_count(), 0);

        assert!(!manager.is_full());
    }

    #[test]
    fn peer_can_be_added() {
        let mut manager = PeerManager::new(peer_id(1)).expect("manager");

        manager
            .add_peer(peer_id(2), address(10001))
            .expect("add peer");

        assert_eq!(manager.peer_count(), 1);

        assert!(manager.contains(&peer_id(2)));
    }

    #[test]
    fn peer_information_is_stored() {
        let mut manager = PeerManager::new(peer_id(1)).expect("manager");

        manager
            .add_peer(peer_id(2), address(10002))
            .expect("add peer");

        let peer = manager.get(&peer_id(2)).expect("peer");

        assert_eq!(peer.peer_id(), peer_id(2));

        assert_eq!(peer.address(), address(10002));

        assert_eq!(peer.state(), ProtocolState::Connected);
    }

    #[test]
    fn duplicate_peer_is_rejected() {
        let mut manager = PeerManager::new(peer_id(1)).expect("manager");

        manager
            .add_peer(peer_id(2), address(10003))
            .expect("first peer");

        let result = manager.add_peer(peer_id(2), address(10004));

        assert!(result.is_err());
        assert_eq!(manager.peer_count(), 1);
    }

    #[test]
    fn self_peer_is_rejected() {
        let mut manager = PeerManager::new(peer_id(1)).expect("manager");

        let result = manager.add_peer(peer_id(1), address(10005));

        assert!(result.is_err());
        assert_eq!(manager.peer_count(), 0);
    }

    #[test]
    fn zero_peer_is_rejected() {
        let mut manager = PeerManager::new(peer_id(1)).expect("manager");

        let result = manager.add_peer([0u8; 32], address(10006));

        assert!(result.is_err());
        assert_eq!(manager.peer_count(), 0);
    }

    #[test]
    fn peer_limit_is_enforced() {
        let mut manager = PeerManager::with_max_peers(peer_id(1), 2).expect("manager");

        manager
            .add_peer(peer_id(2), address(10007))
            .expect("peer 2");

        manager
            .add_peer(peer_id(3), address(10008))
            .expect("peer 3");

        assert!(manager.is_full());

        let result = manager.add_peer(peer_id(4), address(10009));

        assert!(result.is_err());

        assert_eq!(manager.peer_count(), 2);
    }

    #[test]
    fn peer_can_be_removed() {
        let mut manager = PeerManager::new(peer_id(1)).expect("manager");

        manager.add_peer(peer_id(2), address(10010)).expect("peer");

        let removed = manager.remove_peer(&peer_id(2));

        assert!(removed.is_some());

        assert_eq!(manager.peer_count(), 0);

        assert!(!manager.contains(&peer_id(2)));
    }

    #[test]
    fn removing_unknown_peer_returns_none() {
        let mut manager = PeerManager::new(peer_id(1)).expect("manager");

        assert!(manager.remove_peer(&peer_id(2)).is_none());
    }

    #[test]
    fn peer_state_can_be_updated() {
        let mut manager = PeerManager::new(peer_id(1)).expect("manager");

        manager.add_peer(peer_id(2), address(10011)).expect("peer");

        manager
            .set_peer_state(&peer_id(2), ProtocolState::Established)
            .expect("state");

        assert_eq!(
            manager.get(&peer_id(2)).unwrap().state(),
            ProtocolState::Established
        );
    }

    #[test]
    fn unknown_peer_state_update_is_rejected() {
        let mut manager = PeerManager::new(peer_id(1)).expect("manager");

        let result = manager.set_peer_state(&peer_id(2), ProtocolState::Established);

        assert!(result.is_err());
    }

    #[test]
    fn connected_peers_returns_active_peers() {
        let mut manager = PeerManager::new(peer_id(1)).expect("manager");

        manager
            .add_peer(peer_id(2), address(10012))
            .expect("peer 2");

        manager
            .add_peer(peer_id(3), address(10013))
            .expect("peer 3");

        manager
            .set_peer_state(&peer_id(3), ProtocolState::Closed)
            .expect("close peer");

        let connected = manager.connected_peers();

        assert_eq!(connected.len(), 1);

        assert_eq!(connected[0], peer_id(2));
    }

    #[test]
    fn peer_ids_returns_all_peers() {
        let mut manager = PeerManager::new(peer_id(1)).expect("manager");

        manager
            .add_peer(peer_id(2), address(10014))
            .expect("peer 2");

        manager
            .add_peer(peer_id(3), address(10015))
            .expect("peer 3");

        let ids = manager.peer_ids();

        assert_eq!(ids.len(), 2);

        assert!(ids.contains(&peer_id(2)));

        assert!(ids.contains(&peer_id(3)));
    }

    #[test]
    fn clear_removes_all_peers() {
        let mut manager = PeerManager::new(peer_id(1)).expect("manager");

        manager
            .add_peer(peer_id(2), address(10016))
            .expect("peer 2");

        manager
            .add_peer(peer_id(3), address(10017))
            .expect("peer 3");

        manager.clear();

        assert_eq!(manager.peer_count(), 0);
    }
}
