use std::collections::HashSet;
use std::net::SocketAddr;

use crate::p2p::{PeerId, PeerManager};

pub const MAX_DISCOVERY_PEERS: usize = 256;
pub const MAX_DISCOVERY_RESULTS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPeer {
    pub id: PeerId,
    pub address: SocketAddr,
}

impl DiscoveredPeer {
    pub fn new(id: PeerId, address: SocketAddr) -> Result<Self, String> {
        validate_peer_id(&id)?;
        validate_socket_address(&address)?;

        Ok(Self { id, address })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerDiscovery {
    local_node_id: PeerId,
    max_results: usize,
}

impl PeerDiscovery {
    pub fn new(local_node_id: PeerId, max_results: usize) -> Result<Self, String> {
        validate_peer_id(&local_node_id)?;

        if max_results == 0 {
            return Err("maximum discovery results cannot be zero".to_string());
        }

        if max_results > MAX_DISCOVERY_RESULTS {
            return Err("maximum discovery results exceeds protocol limit".to_string());
        }

        Ok(Self {
            local_node_id,
            max_results,
        })
    }

    pub fn local_node_id(&self) -> PeerId {
        self.local_node_id
    }

    pub fn max_results(&self) -> usize {
        self.max_results
    }

    pub fn discover(&self, peers: &[DiscoveredPeer]) -> Result<Vec<DiscoveredPeer>, String> {
        if peers.len() > MAX_DISCOVERY_PEERS {
            return Err("too many discovered peers".to_string());
        }

        let mut result = Vec::new();
        let mut seen = HashSet::new();

        for peer in peers {
            validate_peer_id(&peer.id)?;
            validate_socket_address(&peer.address)?;

            if peer.id == self.local_node_id {
                continue;
            }

            if !seen.insert(peer.id) {
                continue;
            }

            result.push(peer.clone());

            if result.len() >= self.max_results {
                break;
            }
        }

        Ok(result)
    }

    pub fn discover_from_manager(
        &self,
        manager: &PeerManager,
    ) -> Result<Vec<DiscoveredPeer>, String> {
        let mut candidates = Vec::new();

        for peer in manager.peers() {
            if peer.id == self.local_node_id {
                continue;
            }

            candidates.push(DiscoveredPeer::new(peer.id, peer.address)?);
        }

        self.discover(&candidates)
    }

    pub fn add_discovered_peer(
        &self,
        manager: &mut PeerManager,
        peer: DiscoveredPeer,
    ) -> Result<(), String> {
        validate_peer_id(&peer.id)?;
        validate_socket_address(&peer.address)?;

        if peer.id == self.local_node_id {
            return Err("cannot add local node as peer".to_string());
        }

        if manager.contains(&peer.id) {
            return Err("peer already exists".to_string());
        }

        manager.add_peer(peer.id, peer.address)
    }

    pub fn add_many(
        &self,
        manager: &mut PeerManager,
        peers: &[DiscoveredPeer],
    ) -> Result<usize, String> {
        if peers.len() > MAX_DISCOVERY_PEERS {
            return Err("too many discovered peers".to_string());
        }

        let mut added = 0usize;

        for peer in peers {
            match self.add_discovered_peer(manager, peer.clone()) {
                Ok(()) => {
                    added = added
                        .checked_add(1)
                        .ok_or_else(|| "peer counter overflow".to_string())?;
                }

                Err(error) if error == "peer already exists" => {
                    continue;
                }

                Err(error) => {
                    return Err(error);
                }
            }
        }

        Ok(added)
    }
}

pub fn validate_peer_id(peer_id: &PeerId) -> Result<(), String> {
    if *peer_id == [0u8; 32] {
        return Err("peer id cannot be zero".to_string());
    }

    Ok(())
}

pub fn validate_socket_address(address: &SocketAddr) -> Result<(), String> {
    if address.port() == 0 {
        return Err("peer port cannot be zero".to_string());
    }

    Ok(())
}

pub fn validate_discovered_peer(
    local_node_id: &PeerId,
    peer: &DiscoveredPeer,
) -> Result<(), String> {
    validate_peer_id(local_node_id)?;
    validate_peer_id(&peer.id)?;
    validate_socket_address(&peer.address)?;

    if local_node_id == &peer.id {
        return Err("discovered peer cannot be local node".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer_id(value: u8) -> PeerId {
        [value; 32]
    }

    fn address(port: u16) -> SocketAddr {
        format!("127.0.0.1:{port}").parse().expect("valid address")
    }

    fn discovered_peer(id: u8, port: u16) -> DiscoveredPeer {
        DiscoveredPeer::new(peer_id(id), address(port)).expect("valid discovered peer")
    }

    #[test]
    fn valid_discovered_peer_is_created() {
        let peer = discovered_peer(2, 9002);

        assert_eq!(peer.id, peer_id(2));
        assert_eq!(peer.address, address(9002));
    }

    #[test]
    fn zero_peer_id_is_rejected() {
        assert!(DiscoveredPeer::new([0u8; 32], address(9001)).is_err());
    }

    #[test]
    fn zero_port_is_rejected() {
        let address = "127.0.0.1:0"
            .parse::<SocketAddr>()
            .expect("valid socket address");

        assert!(DiscoveredPeer::new(peer_id(2), address).is_err());
    }

    #[test]
    fn discovery_rejects_zero_local_id() {
        assert!(PeerDiscovery::new([0u8; 32], 10).is_err());
    }

    #[test]
    fn discovery_rejects_zero_max_results() {
        assert!(PeerDiscovery::new(peer_id(1), 0).is_err());
    }

    #[test]
    fn discovery_rejects_excessive_max_results() {
        assert!(PeerDiscovery::new(peer_id(1), MAX_DISCOVERY_RESULTS + 1).is_err());
    }

    #[test]
    fn self_peer_is_not_returned() {
        let discovery = PeerDiscovery::new(peer_id(1), 10).expect("discovery");

        let peers = vec![discovered_peer(1, 9001), discovered_peer(2, 9002)];

        let result = discovery.discover(&peers).expect("discover");

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, peer_id(2));
    }

    #[test]
    fn duplicate_peers_are_removed() {
        let discovery = PeerDiscovery::new(peer_id(1), 10).expect("discovery");

        let peers = vec![
            discovered_peer(2, 9002),
            discovered_peer(2, 9002),
            discovered_peer(3, 9003),
        ];

        let result = discovery.discover(&peers).expect("discover");

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, peer_id(2));
        assert_eq!(result[1].id, peer_id(3));
    }

    #[test]
    fn discovery_result_limit_is_enforced() {
        let discovery = PeerDiscovery::new(peer_id(1), 2).expect("discovery");

        let peers = vec![
            discovered_peer(2, 9002),
            discovered_peer(3, 9003),
            discovered_peer(4, 9004),
        ];

        let result = discovery.discover(&peers).expect("discover");

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn too_many_discovered_peers_are_rejected() {
        let discovery = PeerDiscovery::new(peer_id(1), 10).expect("discovery");

        let peers = vec![discovered_peer(2, 9002); MAX_DISCOVERY_PEERS + 1];

        assert!(discovery.discover(&peers).is_err());
    }

    #[test]
    fn peer_can_be_added_to_manager() {
        let discovery = PeerDiscovery::new(peer_id(1), 10).expect("discovery");

        let mut manager = PeerManager::new(8).expect("manager");

        let peer = discovered_peer(2, 9002);

        discovery
            .add_discovered_peer(&mut manager, peer)
            .expect("peer should be added");

        assert_eq!(manager.len(), 1);
        assert!(manager.contains(&peer_id(2)));
    }

    #[test]
    fn self_peer_cannot_be_added() {
        let discovery = PeerDiscovery::new(peer_id(1), 10).expect("discovery");

        let mut manager = PeerManager::new(8).expect("manager");

        let peer = discovered_peer(1, 9001);

        assert!(discovery.add_discovered_peer(&mut manager, peer).is_err());

        assert!(manager.is_empty());
    }

    #[test]
    fn duplicate_peer_is_not_added_twice() {
        let discovery = PeerDiscovery::new(peer_id(1), 10).expect("discovery");

        let mut manager = PeerManager::new(8).expect("manager");

        let peer = discovered_peer(2, 9002);

        discovery
            .add_discovered_peer(&mut manager, peer.clone())
            .expect("first add");

        assert!(discovery.add_discovered_peer(&mut manager, peer).is_err());

        assert_eq!(manager.len(), 1);
    }

    #[test]
    fn add_many_adds_valid_new_peers() {
        let discovery = PeerDiscovery::new(peer_id(1), 10).expect("discovery");

        let mut manager = PeerManager::new(8).expect("manager");

        let peers = vec![
            discovered_peer(2, 9002),
            discovered_peer(3, 9003),
            discovered_peer(4, 9004),
        ];

        let added = discovery.add_many(&mut manager, &peers).expect("add many");

        assert_eq!(added, 3);
        assert_eq!(manager.len(), 3);
    }

    #[test]
    fn add_many_ignores_existing_peers() {
        let discovery = PeerDiscovery::new(peer_id(1), 10).expect("discovery");

        let mut manager = PeerManager::new(8).expect("manager");

        manager
            .add_peer(peer_id(2), address(9002))
            .expect("existing peer");

        let peers = vec![discovered_peer(2, 9002), discovered_peer(3, 9003)];

        let added = discovery.add_many(&mut manager, &peers).expect("add many");

        assert_eq!(added, 1);
        assert_eq!(manager.len(), 2);
    }

    #[test]
    fn manager_discovery_works() {
        let discovery = PeerDiscovery::new(peer_id(1), 10).expect("discovery");

        let mut manager = PeerManager::new(8).expect("manager");

        manager.add_peer(peer_id(2), address(9002)).expect("peer 2");

        manager.add_peer(peer_id(3), address(9003)).expect("peer 3");

        let result = discovery.discover_from_manager(&manager).expect("discover");

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn peer_pair_validation_rejects_self() {
        assert!(validate_discovered_peer(&peer_id(1), &discovered_peer(1, 9001)).is_err());
    }

    #[test]
    fn valid_peer_pair_is_accepted() {
        assert!(validate_discovered_peer(&peer_id(1), &discovered_peer(2, 9002)).is_ok());
    }
}
