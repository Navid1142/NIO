use crate::block::Block;
use crate::transaction::Transaction;
use std::collections::HashMap;
use std::net::SocketAddr;

pub type PeerId = [u8; 32];

const PROTOCOL_VERSION: u32 = 1;
const MAX_BLOCK_MESSAGE_SIZE: usize = 2_000_000;
const MAX_TRANSACTION_MESSAGE_SIZE: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub id: PeerId,
    pub address: SocketAddr,
    pub connected: bool,
}

#[derive(Debug, Clone)]
pub enum NetworkMessage {
    Ping { nonce: u64 },

    Pong { nonce: u64 },

    GetPeers,

    Peers { peers: Vec<PeerId> },

    GetBlocks { start_height: u64, count: u32 },

    Block { block: Block },

    Transaction { transaction: Transaction },
}

impl NetworkMessage {
    pub fn protocol_version() -> u32 {
        PROTOCOL_VERSION
    }

    pub fn message_name(&self) -> &'static str {
        match self {
            Self::Ping { .. } => "ping",
            Self::Pong { .. } => "pong",
            Self::GetPeers => "get_peers",
            Self::Peers { .. } => "peers",
            Self::GetBlocks { .. } => "get_blocks",
            Self::Block { .. } => "block",
            Self::Transaction { .. } => "transaction",
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Ping { .. } => Ok(()),

            Self::Pong { .. } => Ok(()),

            Self::GetPeers => Ok(()),

            Self::Peers { peers } => {
                if peers.len() > 256 {
                    return Err("too many peers in network message".to_string());
                }

                Ok(())
            }

            Self::GetBlocks {
                start_height: _,
                count,
            } => {
                if *count == 0 {
                    return Err("block request count cannot be zero".to_string());
                }

                if *count > 2_000 {
                    return Err("block request exceeds maximum count".to_string());
                }

                Ok(())
            }

            Self::Block { block } => {
                if block.transactions.len() > 10_000 {
                    return Err("block contains too many transactions".to_string());
                }

                Ok(())
            }

            Self::Transaction { transaction } => {
                if transaction.inputs.len() > 1_000 {
                    return Err("transaction contains too many inputs".to_string());
                }

                if transaction.outputs.len() > 1_000 {
                    return Err("transaction contains too many outputs".to_string());
                }

                Ok(())
            }
        }
    }

    pub fn estimated_size(&self) -> usize {
        match self {
            Self::Ping { .. } => 16,

            Self::Pong { .. } => 16,

            Self::GetPeers => 8,

            Self::Peers { peers } => 8 + peers.len() * 32,

            Self::GetBlocks { .. } => 24,

            Self::Block { block } => {
                let tx_size = block
                    .transactions
                    .iter()
                    .map(|tx| 64 + tx.inputs.len() * 160 + tx.outputs.len() * 100)
                    .sum::<usize>();

                256 + tx_size
            }

            Self::Transaction { transaction } => {
                128 + transaction.inputs.len() * 160 + transaction.outputs.len() * 100
            }
        }
    }

    pub fn is_within_size_limit(&self) -> bool {
        match self {
            Self::Block { .. } => self.estimated_size() <= MAX_BLOCK_MESSAGE_SIZE,

            Self::Transaction { .. } => self.estimated_size() <= MAX_TRANSACTION_MESSAGE_SIZE,

            _ => true,
        }
    }

    pub fn validate_for_network(&self) -> Result<(), String> {
        self.validate()?;

        if !self.is_within_size_limit() {
            return Err(format!(
                "{} message exceeds network size limit",
                self.message_name()
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerManager {
    peers: HashMap<PeerId, Peer>,
    max_peers: usize,
}

impl PeerManager {
    pub fn new(max_peers: usize) -> Result<Self, String> {
        if max_peers == 0 {
            return Err("max peers cannot be zero".to_string());
        }

        Ok(Self {
            peers: HashMap::new(),
            max_peers,
        })
    }

    pub fn len(&self) -> usize {
        self.peers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }

    pub fn max_peers(&self) -> usize {
        self.max_peers
    }

    pub fn contains(&self, id: &PeerId) -> bool {
        self.peers.contains_key(id)
    }

    pub fn get(&self, id: &PeerId) -> Option<&Peer> {
        self.peers.get(id)
    }

    pub fn peers(&self) -> impl Iterator<Item = &Peer> {
        self.peers.values()
    }

    pub fn add_peer(&mut self, id: PeerId, address: SocketAddr) -> Result<(), String> {
        if self.contains(&id) {
            return Err("peer already exists".to_string());
        }

        if self.peers.len() >= self.max_peers {
            return Err("peer limit reached".to_string());
        }

        let peer = Peer {
            id,
            address,
            connected: false,
        };

        self.peers.insert(id, peer);

        Ok(())
    }

    pub fn connect(&mut self, id: &PeerId) -> Result<(), String> {
        let peer = self
            .peers
            .get_mut(id)
            .ok_or_else(|| "peer does not exist".to_string())?;

        if peer.connected {
            return Err("peer is already connected".to_string());
        }

        peer.connected = true;

        Ok(())
    }

    pub fn disconnect(&mut self, id: &PeerId) -> Result<(), String> {
        let peer = self
            .peers
            .get_mut(id)
            .ok_or_else(|| "peer does not exist".to_string())?;

        if !peer.connected {
            return Err("peer is already disconnected".to_string());
        }

        peer.connected = false;

        Ok(())
    }

    pub fn remove_peer(&mut self, id: &PeerId) -> Result<Peer, String> {
        self.peers
            .remove(id)
            .ok_or_else(|| "peer does not exist".to_string())
    }

    pub fn connected_count(&self) -> usize {
        self.peers.values().filter(|peer| peer.connected).count()
    }
}

impl Default for PeerManager {
    fn default() -> Self {
        Self::new(32).expect("default peer limit must be valid")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer_id(value: u8) -> PeerId {
        [value; 32]
    }

    fn address(port: u16) -> SocketAddr {
        format!("127.0.0.1:{port}")
            .parse()
            .expect("valid socket address")
    }

    #[test]
    fn peer_manager_starts_empty() {
        let manager = PeerManager::new(8).expect("manager");

        assert!(manager.is_empty());
        assert_eq!(manager.len(), 0);
    }

    #[test]
    fn zero_peer_limit_is_rejected() {
        assert!(PeerManager::new(0).is_err());
    }

    #[test]
    fn peer_can_be_added() {
        let mut manager = PeerManager::new(8).expect("manager");

        manager
            .add_peer(peer_id(1), address(9001))
            .expect("peer should be added");

        assert_eq!(manager.len(), 1);
        assert!(manager.contains(&peer_id(1)));
    }

    #[test]
    fn duplicate_peer_is_rejected() {
        let mut manager = PeerManager::new(8).expect("manager");

        manager
            .add_peer(peer_id(1), address(9001))
            .expect("first peer");

        assert!(manager.add_peer(peer_id(1), address(9002)).is_err());
    }

    #[test]
    fn peer_limit_is_enforced() {
        let mut manager = PeerManager::new(2).expect("manager");

        manager.add_peer(peer_id(1), address(9001)).expect("peer 1");

        manager.add_peer(peer_id(2), address(9002)).expect("peer 2");

        assert!(manager.add_peer(peer_id(3), address(9003)).is_err());
    }

    #[test]
    fn peer_can_connect() {
        let mut manager = PeerManager::new(8).expect("manager");

        manager.add_peer(peer_id(1), address(9001)).expect("peer");

        manager.connect(&peer_id(1)).expect("connect");

        assert_eq!(manager.connected_count(), 1);
        assert!(manager.get(&peer_id(1)).unwrap().connected);
    }

    #[test]
    fn peer_can_disconnect() {
        let mut manager = PeerManager::new(8).expect("manager");

        manager.add_peer(peer_id(1), address(9001)).expect("peer");

        manager.connect(&peer_id(1)).expect("connect");
        manager.disconnect(&peer_id(1)).expect("disconnect");

        assert_eq!(manager.connected_count(), 0);
    }

    #[test]
    fn peer_can_be_removed() {
        let mut manager = PeerManager::new(8).expect("manager");

        manager.add_peer(peer_id(1), address(9001)).expect("peer");

        manager.remove_peer(&peer_id(1)).expect("remove");

        assert!(manager.is_empty());
    }

    #[test]
    fn ping_message_is_valid() {
        let message = NetworkMessage::Ping { nonce: 123 };

        assert_eq!(message.message_name(), "ping");
        assert!(message.validate_for_network().is_ok());
    }

    #[test]
    fn pong_message_is_valid() {
        let message = NetworkMessage::Pong { nonce: 456 };

        assert!(message.validate_for_network().is_ok());
    }

    #[test]
    fn get_peers_message_is_valid() {
        let message = NetworkMessage::GetPeers;

        assert!(message.validate_for_network().is_ok());
    }

    #[test]
    fn peers_message_is_valid() {
        let message = NetworkMessage::Peers {
            peers: vec![peer_id(1), peer_id(2), peer_id(3)],
        };

        assert!(message.validate_for_network().is_ok());
    }

    #[test]
    fn too_many_peers_are_rejected() {
        let message = NetworkMessage::Peers {
            peers: vec![[1u8; 32]; 257],
        };

        assert!(message.validate_for_network().is_err());
    }

    #[test]
    fn block_request_must_have_positive_count() {
        let message = NetworkMessage::GetBlocks {
            start_height: 0,
            count: 0,
        };

        assert!(message.validate_for_network().is_err());
    }

    #[test]
    fn block_request_limit_is_enforced() {
        let message = NetworkMessage::GetBlocks {
            start_height: 0,
            count: 2_001,
        };

        assert!(message.validate_for_network().is_err());
    }

    #[test]
    fn valid_block_request_is_accepted() {
        let message = NetworkMessage::GetBlocks {
            start_height: 100,
            count: 100,
        };

        assert!(message.validate_for_network().is_ok());
    }

    #[test]
    fn protocol_version_is_one() {
        assert_eq!(NetworkMessage::protocol_version(), 1);
    }

    #[test]
    fn message_names_are_stable() {
        assert_eq!(NetworkMessage::Ping { nonce: 1 }.message_name(), "ping");

        assert_eq!(NetworkMessage::Pong { nonce: 1 }.message_name(), "pong");

        assert_eq!(NetworkMessage::GetPeers.message_name(), "get_peers");
    }

    #[test]
    fn network_message_size_is_bounded() {
        let message = NetworkMessage::Ping { nonce: 1 };

        assert!(message.is_within_size_limit());
        assert!(message.estimated_size() < 1_000);
    }
}
