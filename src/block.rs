use crate::economy::Economy;
use crate::fee::FeeAccounting;
use crate::transaction::Transaction;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct BlockHeader {
    pub version: u32,
    pub height: u64,
    pub previous_hash: [u8; 32],
    pub merkle_root: [u8; 32],
    pub timestamp: u64,
    pub difficulty: u32,
    pub nonce: u64,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub miner_reward: u64,
    pub transactions: Vec<Transaction>,
}

impl Block {
    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    pub fn new(
        version: u32,
        height: u64,
        previous_hash: [u8; 32],
        timestamp: u64,
        difficulty: u32,
        nonce: u64,
        miner_reward: u64,
    ) -> Self {
        Self {
            header: BlockHeader {
                version,
                height,
                previous_hash,
                merkle_root: [0u8; 32],
                timestamp,
                difficulty,
                nonce,
            },
            miner_reward,
            transactions: Vec::new(),
        }
    }

    // ============================================================
    // CONSTRUCTOR WITH TRANSACTIONS
    // ============================================================

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_transactions(
        version: u32,
        height: u64,
        previous_hash: [u8; 32],
        timestamp: u64,
        difficulty: u32,
        nonce: u64,
        miner_reward: u64,
        transactions: Vec<Transaction>,
    ) -> Self {
        let merkle_root = Self::calculate_merkle_root(&transactions);

        Self {
            header: BlockHeader {
                version,
                height,
                previous_hash,
                merkle_root,
                timestamp,
                difficulty,
                nonce,
            },
            miner_reward,
            transactions,
        }
    }

    // ============================================================
    // MERKLE ROOT
    // ============================================================

    pub fn calculate_merkle_root(transactions: &[Transaction]) -> [u8; 32] {
        if transactions.is_empty() {
            return [0u8; 32];
        }

        let mut hashes: Vec<[u8; 32]> = transactions.iter().map(|tx| tx.id()).collect();

        while hashes.len() > 1 {
            if !hashes.len().is_multiple_of(2) {
                let last = *hashes.last().expect("hash list cannot be empty");

                hashes.push(last);
            }

            let mut next_level = Vec::with_capacity(hashes.len() / 2);

            for pair in hashes.chunks(2) {
                let parent = Self::hash_pair(&pair[0], &pair[1]);

                next_level.push(parent);
            }

            hashes = next_level;
        }

        hashes[0]
    }

    fn hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
        let mut first = Sha256::new();

        first.update(left);
        first.update(right);

        let intermediate = first.finalize();

        let mut second = Sha256::new();

        second.update(intermediate);

        let result = second.finalize();

        let mut hash = [0u8; 32];

        hash.copy_from_slice(&result);

        hash
    }

    pub fn calculated_merkle_root(&self) -> [u8; 32] {
        Self::calculate_merkle_root(&self.transactions)
    }

    pub fn has_valid_merkle_root(&self) -> bool {
        self.header.merkle_root == self.calculated_merkle_root()
    }

    // ============================================================
    // BLOCK HASH
    // ============================================================

    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();

        hasher.update(self.header.version.to_le_bytes());
        hasher.update(self.header.height.to_le_bytes());
        hasher.update(self.header.previous_hash);
        hasher.update(self.header.merkle_root);
        hasher.update(self.header.timestamp.to_le_bytes());
        hasher.update(self.header.difficulty.to_le_bytes());
        hasher.update(self.header.nonce.to_le_bytes());
        hasher.update(self.miner_reward.to_le_bytes());

        let result = hasher.finalize();

        let mut hash = [0u8; 32];

        hash.copy_from_slice(&result);

        hash
    }

    // ============================================================
    // ECONOMY
    // ============================================================

    pub fn expected_reward(&self, current_mining_supply: u64) -> u64 {
        Economy::block_reward(self.header.height, current_mining_supply)
    }

    pub fn has_valid_reward(&self, current_mining_supply: u64) -> bool {
        self.miner_reward == self.expected_reward(current_mining_supply)
    }

    // ============================================================
    // FEE ACCOUNTING
    // ============================================================

    pub fn total_transaction_fees(&self) -> Result<u64, String> {
        FeeAccounting::total_fees(&self.transactions)
    }

    pub fn expected_miner_payout(&self, current_mining_supply: u64) -> Result<u64, String> {
        let base_reward = self.expected_reward(current_mining_supply);

        let total_fees = self.total_transaction_fees()?;

        FeeAccounting::miner_payout(base_reward, total_fees)
    }

    pub fn has_valid_miner_payout(&self, current_mining_supply: u64) -> bool {
        match self.expected_miner_payout(current_mining_supply) {
            Ok(expected) => self.miner_reward == expected,

            Err(_) => false,
        }
    }

    // ============================================================
    // BASIC VALIDATION
    // ============================================================

    pub fn validate_basic(&self, current_mining_supply: u64) -> bool {
        if !Economy::is_supply_valid(current_mining_supply) {
            return false;
        }

        if !self.has_valid_reward(current_mining_supply) {
            return false;
        }

        if !self.has_valid_merkle_root() {
            return false;
        }

        true
    }
}

// ================================================================
// TESTS
// ================================================================

#[cfg(test)]
mod tests {
    use super::*;

    use crate::economy::{INITIAL_REWARD_UNITS, MINING_ALLOCATION_UNITS};

    use crate::transaction::{Transaction, TransactionInput, TransactionOutput};

    // ============================================================
    // TEST HELPERS
    // ============================================================

    fn test_hash() -> [u8; 32] {
        [0u8; 32]
    }

    fn test_transaction(value: u8, fee: u64) -> Transaction {
        Transaction::new(
            1,
            vec![TransactionInput {
                previous_output: [value; 32],
                output_index: 0,
                public_key: Vec::new(),
                signature: Vec::new(),
            }],
            vec![TransactionOutput {
                amount: 100,
                recipient: vec![2u8; 33],
            }],
            fee,
        )
    }

    // ============================================================
    // BASIC BLOCK TESTS
    // ============================================================

    #[test]
    fn block_can_be_created() {
        let block = Block::new(1, 0, test_hash(), 1_000, 1, 0, INITIAL_REWARD_UNITS);

        assert_eq!(block.header.height, 0);

        assert_eq!(block.miner_reward, INITIAL_REWARD_UNITS);

        assert!(block.transactions.is_empty());

        assert_eq!(block.header.merkle_root, [0u8; 32]);
    }

    #[test]
    fn block_hash_is_32_bytes() {
        let block = Block::new(1, 0, test_hash(), 1_000, 1, 0, INITIAL_REWARD_UNITS);

        assert_eq!(block.hash().len(), 32);
    }

    #[test]
    fn valid_reward_is_accepted() {
        let block = Block::new(1, 0, test_hash(), 1_000, 1, 0, INITIAL_REWARD_UNITS);

        assert!(block.validate_basic(0));
    }

    #[test]
    fn excessive_reward_is_rejected() {
        let block = Block::new(1, 0, test_hash(), 1_000, 1, 0, INITIAL_REWARD_UNITS + 1);

        assert!(!block.validate_basic(0));
    }

    #[test]
    fn zero_reward_is_rejected_before_cap() {
        let block = Block::new(1, 0, test_hash(), 1_000, 1, 0, 0);

        assert!(!block.validate_basic(0));
    }

    #[test]
    fn reward_is_zero_at_cap() {
        let block = Block::new(1, 0, test_hash(), 1_000, 1, 0, 0);

        assert!(block.validate_basic(MINING_ALLOCATION_UNITS));
    }

    #[test]
    fn changing_nonce_changes_hash() {
        let block_a = Block::new(1, 0, test_hash(), 1_000, 1, 0, INITIAL_REWARD_UNITS);

        let block_b = Block::new(1, 0, test_hash(), 1_000, 1, 1, INITIAL_REWARD_UNITS);

        assert_ne!(block_a.hash(), block_b.hash());
    }

    // ============================================================
    // MERKLE TESTS
    // ============================================================

    #[test]
    fn empty_transactions_have_zero_merkle_root() {
        let transactions: Vec<Transaction> = Vec::new();

        assert_eq!(Block::calculate_merkle_root(&transactions), [0u8; 32]);
    }

    #[test]
    fn one_transaction_merkle_root_equals_transaction_id() {
        let tx = test_transaction(1, 1);

        let transactions = vec![tx.clone()];

        let root = Block::calculate_merkle_root(&transactions);

        assert_eq!(root, tx.id());
    }

    #[test]
    fn transactions_are_stored_in_block() {
        let tx1 = test_transaction(1, 10);

        let tx2 = test_transaction(2, 20);

        let block = Block::new_with_transactions(
            1,
            1,
            test_hash(),
            1_060,
            1,
            0,
            INITIAL_REWARD_UNITS,
            vec![tx1, tx2],
        );

        assert_eq!(block.transactions.len(), 2);

        assert!(block.has_valid_merkle_root());
    }

    #[test]
    fn changing_transaction_changes_merkle_root() {
        let tx1 = test_transaction(1, 10);

        let tx2 = test_transaction(2, 20);

        let root_a = Block::calculate_merkle_root(std::slice::from_ref(&tx1));

        let root_b = Block::calculate_merkle_root(&[tx2]);

        assert_ne!(root_a, root_b);
    }

    #[test]
    fn changing_transaction_changes_block_hash() {
        let tx1 = test_transaction(1, 10);

        let tx2 = test_transaction(2, 20);

        let block_a = Block::new_with_transactions(
            1,
            1,
            test_hash(),
            1_060,
            1,
            0,
            INITIAL_REWARD_UNITS,
            vec![tx1],
        );

        let block_b = Block::new_with_transactions(
            1,
            1,
            test_hash(),
            1_060,
            1,
            0,
            INITIAL_REWARD_UNITS,
            vec![tx2],
        );

        assert_ne!(block_a.hash(), block_b.hash());
    }

    #[test]
    fn changing_transaction_invalidates_merkle_root() {
        let tx1 = test_transaction(1, 10);

        let tx2 = test_transaction(2, 20);

        let mut block = Block::new_with_transactions(
            1,
            1,
            test_hash(),
            1_060,
            1,
            0,
            INITIAL_REWARD_UNITS,
            vec![tx1],
        );

        assert!(block.has_valid_merkle_root());

        block.transactions[0] = tx2;

        assert!(!block.has_valid_merkle_root());
    }

    #[test]
    fn odd_transaction_count_is_supported() {
        let tx1 = test_transaction(1, 10);

        let tx2 = test_transaction(2, 20);

        let tx3 = test_transaction(3, 30);

        let root = Block::calculate_merkle_root(&[tx1, tx2, tx3]);

        assert_ne!(root, [0u8; 32]);
    }

    // ============================================================
    // FEE TESTS
    // ============================================================

    #[test]
    fn block_total_fees_are_calculated() {
        let tx1 = test_transaction(1, 10);

        let tx2 = test_transaction(2, 25);

        let tx3 = test_transaction(3, 5);

        let block = Block::new_with_transactions(
            1,
            1,
            test_hash(),
            1_060,
            1,
            0,
            INITIAL_REWARD_UNITS + 40,
            vec![tx1, tx2, tx3],
        );

        assert_eq!(
            block
                .total_transaction_fees()
                .expect("fee calculation should succeed"),
            40
        );
    }

    #[test]
    fn miner_payout_equals_reward_plus_fees() {
        let tx1 = test_transaction(1, 10);

        let tx2 = test_transaction(2, 25);

        let block = Block::new_with_transactions(
            1,
            1,
            test_hash(),
            1_060,
            1,
            0,
            INITIAL_REWARD_UNITS + 35,
            vec![tx1, tx2],
        );

        assert_eq!(
            block
                .expected_miner_payout(0)
                .expect("payout should calculate"),
            INITIAL_REWARD_UNITS + 35
        );
    }

    #[test]
    fn valid_miner_payout_is_accepted() {
        let tx = test_transaction(1, 50);

        let block = Block::new_with_transactions(
            1,
            1,
            test_hash(),
            1_060,
            1,
            0,
            INITIAL_REWARD_UNITS + 50,
            vec![tx],
        );

        assert!(block.has_valid_miner_payout(0));
    }

    #[test]
    fn excessive_miner_payout_is_rejected() {
        let tx = test_transaction(1, 50);

        let block = Block::new_with_transactions(
            1,
            1,
            test_hash(),
            1_060,
            1,
            0,
            INITIAL_REWARD_UNITS + 51,
            vec![tx],
        );

        assert!(!block.has_valid_miner_payout(0));
    }

    #[test]
    fn missing_fee_from_miner_payout_is_rejected() {
        let tx = test_transaction(1, 50);

        let block = Block::new_with_transactions(
            1,
            1,
            test_hash(),
            1_060,
            1,
            0,
            INITIAL_REWARD_UNITS,
            vec![tx],
        );

        assert!(!block.has_valid_miner_payout(0));
    }

    #[test]
    fn zero_fee_block_keeps_base_reward() {
        let tx = test_transaction(1, 0);

        let block = Block::new_with_transactions(
            1,
            1,
            test_hash(),
            1_060,
            1,
            0,
            INITIAL_REWARD_UNITS,
            vec![tx],
        );

        assert_eq!(
            block
                .total_transaction_fees()
                .expect("fee calculation should succeed"),
            0
        );

        assert!(block.has_valid_miner_payout(0));
    }
}
