use crate::block::Block;
use sha2::{Digest, Sha256};

/// Proof-of-Work mining utilities.
pub struct Miner;

impl Miner {
    /// Mine a block by searching for a nonce that satisfies
    /// the block's configured difficulty.
    ///
    /// Difficulty is interpreted as the number of leading
    /// zero bytes required in the SHA-256 block hash.
    pub fn mine(block: &mut Block) -> Result<u64, String> {
        let difficulty = block.header.difficulty as usize;

        if difficulty == 0 {
            return Err("difficulty must be greater than zero".to_string());
        }

        if difficulty > 32 {
            return Err("difficulty cannot exceed 32".to_string());
        }

        let mut nonce = block.header.nonce;

        loop {
            block.header.nonce = nonce;

            if Self::meets_target(block, difficulty) {
                return Ok(nonce);
            }

            nonce = nonce
                .checked_add(1)
                .ok_or_else(|| "nonce overflow".to_string())?;
        }
    }

    /// Check whether a block satisfies the requested PoW target.
    pub fn meets_target(block: &Block, difficulty: usize) -> bool {
        if difficulty == 0 || difficulty > 32 {
            return false;
        }

        let hash = block.hash();

        hash[..difficulty].iter().all(|byte| *byte == 0)
    }

    /// Validate the Proof-of-Work stored in a block.
    pub fn validate(block: &Block) -> bool {
        let difficulty = block.header.difficulty as usize;

        Self::meets_target(block, difficulty)
    }

    /// Calculate SHA-256 for arbitrary data.
    pub fn sha256(data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);

        let result = hasher.finalize();

        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);

        hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::INITIAL_REWARD_UNITS;

    fn test_block(difficulty: u32) -> Block {
        Block::new(1, 1, [0u8; 32], 1_000, difficulty, 0, INITIAL_REWARD_UNITS)
    }

    #[test]
    fn difficulty_zero_is_rejected() {
        let block = test_block(0);

        assert!(!Miner::validate(&block));
    }

    #[test]
    fn difficulty_above_hash_size_is_rejected() {
        let block = test_block(33);

        assert!(!Miner::validate(&block));
    }

    #[test]
    fn sha256_returns_32_bytes() {
        let hash = Miner::sha256(b"NIO");

        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn mining_finds_valid_nonce() {
        let mut block = test_block(1);

        let nonce = Miner::mine(&mut block).expect("mining should succeed");

        assert_eq!(block.header.nonce, nonce);
        assert!(Miner::validate(&block));
    }

    #[test]
    fn mined_block_hash_meets_target() {
        let mut block = test_block(1);

        Miner::mine(&mut block).expect("mining should succeed");

        let hash = block.hash();

        assert_eq!(hash[0], 0);
    }

    #[test]
    fn changing_nonce_changes_pow_result() {
        let block_a = test_block(1);

        let mut block_b = block_a.clone();
        block_b.header.nonce = 1;

        assert_ne!(block_a.hash(), block_b.hash());
    }
}
