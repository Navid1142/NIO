// ================================================================
// NIO BLOCKCHAIN - LIBRARY ROOT
// ================================================================
//
// This file exposes the public modules and important public types
// used by integration tests, binaries, and future components.
//
// ================================================================

// ---------------------------------------------------------------
// CORE BLOCKCHAIN MODULES
// ---------------------------------------------------------------

pub mod block;
pub mod block_builder;
pub mod chain;
pub mod consensus;
pub mod difficulty;
pub mod economy;
pub mod fee;
pub mod genesis;
pub mod mempool;
pub mod mining;
pub mod p2p;
pub mod p2p_codec;
pub mod p2p_discovery;
pub mod p2p_handshake;
pub mod p2p_handshake_session;
pub mod p2p_manager;
pub mod p2p_message;
pub mod p2p_message_codec;
pub mod p2p_node;
pub mod p2p_peer_discovery;
pub mod p2p_peer_manager;
pub mod p2p_protocol;
pub mod p2p_runtime;
pub mod p2p_transport;
pub mod reserve;
pub mod transaction;
pub mod treasury;
pub mod utxo;
pub mod wallet;

// ================================================================
// PUBLIC RE-EXPORTS
// ================================================================

// ---------------------------------------------------------------
// BLOCK
// ---------------------------------------------------------------

pub use crate::block::Block;

// ---------------------------------------------------------------
// BLOCKCHAIN
// ---------------------------------------------------------------

pub use crate::chain::Blockchain;

// ---------------------------------------------------------------
// BLOCK BUILDER
// ---------------------------------------------------------------

pub use crate::block_builder::BlockBuilder;

// ---------------------------------------------------------------
// ECONOMY CONSTANTS
// ---------------------------------------------------------------
//
// The integration tests expect INITIAL_REWARD_UNITS at the crate
// root. Re-export it from the economy module.
//

pub use crate::economy::INITIAL_REWARD_UNITS;

// ================================================================
// COMMON PUBLIC TYPES
// ================================================================

pub use crate::transaction::{Transaction, TransactionId, TransactionInput, TransactionOutput};

pub use crate::utxo::{Utxo, UtxoId, UtxoSet};

// ================================================================
// DIFFICULTY
// ================================================================

pub use crate::difficulty::DifficultyAdjustment;

// ================================================================
// MINING
// ================================================================

pub use crate::mining::Miner;

// ================================================================
// MEMPOOL
// ================================================================

pub use crate::mempool::{Mempool, MempoolConfig, MempoolError};

// ================================================================
// GENESIS
// ================================================================

pub use crate::genesis::GenesisState;

// ================================================================
// WALLET
// ================================================================

pub use crate::wallet::Wallet;

// ================================================================
// FEE
// ================================================================

pub use crate::fee::*;

// ================================================================
// RESERVE / TREASURY
// ================================================================

pub use crate::reserve::*;
pub use crate::treasury::*;

pub mod genesis_utxo;
