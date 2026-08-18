use crate::economy::Economy;
use crate::genesis::GenesisState;
use crate::utxo::{Utxo, UtxoId, UtxoSet};
use sha2::{Digest, Sha256};

pub const TREASURY_PUBLIC_KEY_LENGTH: usize = 33;

/// Creates the canonical Genesis Treasury UTXO.
///
/// NIO monetary policy:
///
/// Total Supply      = 1000 NIO
/// Treasury Reserve  = 100 NIO
/// Mining Allocation = 900 NIO
///
/// The Treasury Reserve is created once at Genesis.
/// It is completely separate from mining rewards.
pub fn create_genesis_utxo(treasury_public_key: Vec<u8>) -> Result<UtxoSet, String> {
    let genesis = GenesisState::new();

    if !genesis.is_valid() {
        return Err("invalid NIO genesis policy".to_string());
    }

    // Treasury uses a compressed secp256k1 public key.
    if treasury_public_key.len() != TREASURY_PUBLIC_KEY_LENGTH {
        return Err("treasury public key must be exactly 33 bytes".to_string());
    }

    // Prevent an invalid all-zero public key.
    if treasury_public_key.iter().all(|&byte| byte == 0) {
        return Err("treasury public key cannot be all zeros".to_string());
    }

    let reserve_amount = Economy::project_reserve();
    let mining_cap = Economy::mining_cap();
    let total_supply = Economy::total_supply();

    if reserve_amount == 0 {
        return Err("treasury reserve cannot be zero".to_string());
    }

    // 100 NIO + 900 NIO = 1000 NIO.
    if reserve_amount.checked_add(mining_cap) != Some(total_supply) {
        return Err("genesis allocations do not equal total supply".to_string());
    }

    // Deterministic Genesis Treasury transaction ID.
    let mut hasher = Sha256::new();

    hasher.update(b"NIO-GENESIS-TREASURY-RESERVE-V1");
    hasher.update(&treasury_public_key);
    hasher.update(reserve_amount.to_le_bytes());

    let digest = hasher.finalize();

    let mut transaction_id = [0u8; 32];
    transaction_id.copy_from_slice(&digest);

    let treasury_utxo = Utxo {
        id: UtxoId {
            transaction_id,
            output_index: 0,
        },
        amount: reserve_amount,
        recipient: treasury_public_key,
    };

    let mut set = UtxoSet::new();

    set.insert(treasury_utxo)
        .map_err(|e| format!("failed to insert treasury genesis UTXO: {:?}", e))?;

    Ok(set)
}

/// Validates the canonical Genesis Treasury UTXO.
pub fn validate_genesis_utxo(set: &UtxoSet, treasury_public_key: &[u8]) -> Result<(), String> {
    let genesis = GenesisState::new();

    if !genesis.is_valid() {
        return Err("invalid NIO genesis policy".to_string());
    }

    if treasury_public_key.len() != TREASURY_PUBLIC_KEY_LENGTH {
        return Err("treasury public key must be exactly 33 bytes".to_string());
    }

    if treasury_public_key.iter().all(|&byte| byte == 0) {
        return Err("treasury public key cannot be all zeros".to_string());
    }

    // Genesis must contain exactly one Treasury UTXO.
    if set.len() != 1 {
        return Err("genesis UTXO set must contain exactly one treasury UTXO".to_string());
    }

    let expected_amount = Economy::project_reserve();

    let found = set
        .iter()
        .find(|utxo| utxo.amount == expected_amount && utxo.recipient == treasury_public_key);

    if found.is_none() {
        return Err("100 NIO treasury genesis UTXO not found".to_string());
    }

    // Final monetary allocation check.
    if Economy::project_reserve().checked_add(Economy::mining_cap())
        != Some(Economy::total_supply())
    {
        return Err("genesis allocation does not equal total supply".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn treasury_public_key() -> Vec<u8> {
        // Placeholder valid-length key for UTXO-layer tests.
        // Cryptographic public-key validation will be added
        // in the transaction/signature layer.
        let mut key = vec![0u8; TREASURY_PUBLIC_KEY_LENGTH];
        key[0] = 0x02;
        key[1] = 0x01;
        key
    }

    #[test]
    fn genesis_creates_treasury_reserve() {
        let key = treasury_public_key();

        let set = create_genesis_utxo(key.clone()).expect("genesis UTXO should be created");

        assert_eq!(set.len(), 1);

        let utxo = set.iter().next().expect("treasury UTXO must exist");

        assert_eq!(utxo.amount, Economy::project_reserve());

        assert_eq!(utxo.amount, 100 * crate::economy::UNITS_PER_NIO);

        assert_eq!(utxo.recipient, key);
    }

    #[test]
    fn treasury_reserve_is_exactly_100_nio() {
        let set =
            create_genesis_utxo(treasury_public_key()).expect("genesis UTXO should be created");

        let utxo = set.iter().next().unwrap();

        assert_eq!(utxo.amount, 100 * crate::economy::UNITS_PER_NIO);
    }

    #[test]
    fn genesis_utxo_is_valid() {
        let key = treasury_public_key();

        let set = create_genesis_utxo(key.clone()).expect("genesis UTXO should be created");

        assert!(validate_genesis_utxo(&set, &key).is_ok());
    }

    #[test]
    fn mining_cap_remains_900_nio() {
        assert_eq!(Economy::mining_cap(), 900 * crate::economy::UNITS_PER_NIO);
    }

    #[test]
    fn treasury_plus_mining_equals_total() {
        assert_eq!(
            Economy::project_reserve() + Economy::mining_cap(),
            Economy::total_supply()
        );
    }

    #[test]
    fn invalid_public_key_length_is_rejected() {
        assert!(create_genesis_utxo(vec![1u8; 32]).is_err());
    }

    #[test]
    fn zero_public_key_is_rejected() {
        assert!(create_genesis_utxo(vec![0u8; TREASURY_PUBLIC_KEY_LENGTH]).is_err());
    }
}
