use crate::p2p::PeerId;
use std::collections::HashMap;
use std::net::SocketAddr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedPeer {
    pub peer_id: PeerId,
    pub address: SocketAddr,
    pub connected: bool,
}

impl ManagedPeer {
    pub fn new(peer_id: PeerId, address: SocketAddr) -> Result<Self, String> {
        if peer_id == [0u8; 32] {
            return Err("peer id cannot be zero".to_string());
        }

        Ok(Self {
            peer_id,
            address,
            connected: false,
        })
    }

    pub fn mark_connected(&mut self) {
        self.connected = true;
    }

    pub fn mark_disconnected(&mut self) {
        self.connected = false;
    }
}

#[derive(Debug, Default)]
pub struct PeerManager {
    peers: HashMap<PeerId, ManagedPeer>,
}

impl PeerManager {
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
        }
    }

    pub fn add_peer(&mut self, peer: ManagedPeer) -> Result<(), String> {
        if peer.peer_id == [0u8; 32] {
            return Err("peer id cannot be zero".to_string());
        }

        if self.peers.contains_key(&peer.peer_id) {
            return Err("peer already exists".to_string());
        }

        self.peers.insert(peer.peer_id, peer);

        Ok(())
    }

    pub fn remove_peer(&mut self, peer_id: &PeerId) -> Option<ManagedPeer> {
        self.peers.remove(peer_id)
    }

    pub fn get_peer(&self, peer_id: &PeerId) -> Option<&ManagedPeer> {
        self.peers.get(peer_id)
    }

    pub fn get_peer_mut(&mut self, peer_id: &PeerId) -> Option<&mut ManagedPeer> {
        self.peers.get_mut(peer_id)
    }

    pub fn contains_peer(&self, peer_id: &PeerId) -> bool {
        self.peers.contains_key(peer_id)
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    pub fn connected_peer_count(&self) -> usize {
        self.peers.values().filter(|peer| peer.connected).count()
    }

    pub fn mark_connected(&mut self, peer_id: &PeerId) -> Result<(), String> {
        let peer = self
            .peers
            .get_mut(peer_id)
            .ok_or_else(|| "peer not found".to_string())?;

        peer.mark_connected();

        Ok(())
    }

    pub fn mark_disconnected(&mut self, peer_id: &PeerId) -> Result<(), String> {
        let peer = self
            .peers
            .get_mut(peer_id)
            .ok_or_else(|| "peer not found".to_string())?;

        peer.mark_disconnected();

        Ok(())
    }

    pub fn peers(&self) -> impl Iterator<Item = &ManagedPeer> {
        self.peers.values()
    }

    pub fn connected_peers(&self) -> impl Iterator<Item = &ManagedPeer> {
        self.peers.values().filter(|peer| peer.connected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer_id(value: u8) -> PeerId {
        [value; 32]
    }

    fn address(port: u16) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], port))
    }

    #[test]
    fn peer_manager_starts_empty() {
        let manager = PeerManager::new();

        assert_eq!(manager.peer_count(), 0);
        assert_eq!(manager.connected_peer_count(), 0);
    }

    #[test]
    fn peer_can_be_created() {
        let peer = ManagedPeer::new(peer_id(1), address(8333)).expect("peer");

        assert_eq!(peer.peer_id, peer_id(1));
        assert_eq!(peer.address, address(8333));
        assert!(!peer.connected);
    }

    #[test]
    fn zero_peer_id_is_rejected() {
        let result = ManagedPeer::new([0u8; 32], address(8333));

        assert!(result.is_err());
    }

    #[test]
    fn peer_can_be_added() {
        let mut manager = PeerManager::new();

        let peer = ManagedPeer::new(peer_id(1), address(8333)).expect("peer");

        manager.add_peer(peer).expect("add peer");

        assert_eq!(manager.peer_count(), 1);
        assert!(manager.contains_peer(&peer_id(1)));
    }

    #[test]
    fn duplicate_peer_is_rejected() {
        let mut manager = PeerManager::new();

        let peer1 = ManagedPeer::new(peer_id(1), address(8333)).expect("peer");

        let peer2 = ManagedPeer::new(peer_id(1), address(8334)).expect("peer");

        manager.add_peer(peer1).expect("first peer");

        assert!(manager.add_peer(peer2).is_err());
    }

    #[test]
    fn peer_can_be_removed() {
        let mut manager = PeerManager::new();

        let peer = ManagedPeer::new(peer_id(1), address(8333)).expect("peer");

        manager.add_peer(peer).expect("add");

        let removed = manager.remove_peer(&peer_id(1));

        assert!(removed.is_some());
        assert_eq!(manager.peer_count(), 0);
    }

    #[test]
    fn peer_can_be_marked_connected() {
        let mut manager = PeerManager::new();

        manager
            .add_peer(ManagedPeer::new(peer_id(1), address(8333)).expect("peer"))
            .expect("add");

        manager.mark_connected(&peer_id(1)).expect("connect");

        assert_eq!(manager.connected_peer_count(), 1);

        assert!(manager.get_peer(&peer_id(1)).unwrap().connected);
    }

    #[test]
    fn peer_can_be_marked_disconnected() {
        let mut manager = PeerManager::new();

        manager
            .add_peer(ManagedPeer::new(peer_id(1), address(8333)).expect("peer"))
            .expect("add");

        manager.mark_connected(&peer_id(1)).expect("connect");

        manager.mark_disconnected(&peer_id(1)).expect("disconnect");

        assert_eq!(manager.connected_peer_count(), 0);
    }

    #[test]
    fn missing_peer_returns_error() {
        let mut manager = PeerManager::new();

        assert!(manager.mark_connected(&peer_id(1)).is_err());

        assert!(manager.mark_disconnected(&peer_id(1)).is_err());
    }

    #[test]
    fn connected_peers_are_filtered() {
        let mut manager = PeerManager::new();

        manager
            .add_peer(ManagedPeer::new(peer_id(1), address(8333)).expect("peer"))
            .expect("add");

        manager
            .add_peer(ManagedPeer::new(peer_id(2), address(8334)).expect("peer"))
            .expect("add");

        manager.mark_connected(&peer_id(1)).expect("connect");

        assert_eq!(manager.connected_peers().count(), 1);
    }
}
