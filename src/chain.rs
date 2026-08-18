use crate::block::Block;
use crate::difficulty::DifficultyAdjustment;
use crate::economy::Economy;
use crate::genesis::GenesisState;
use crate::mining::Miner;
use crate::utxo::UtxoSet;

pub struct Blockchain {
    blocks: Vec<Block>,
    difficulty_adjustment: DifficultyAdjustment,
    utxo_set: UtxoSet,
}

impl Blockchain {
    // ============================================================
    // CONSTRUCTOR
    // ============================================================
    pub fn new() -> Self {
        let genesis_state = GenesisState::new();
        assert!(genesis_state.is_valid(), "NIO genesis state must be valid");
        assert_eq!(
            genesis_state.initial_mining_supply(),
            0,
            "genesis mining supply must start at zero"
        );
        assert_eq!(
            genesis_state.mining_cap(),
            Economy::mining_cap(),
            "genesis mining cap must match economy mining cap"
        );

        let genesis = Block::new(1, 0, [0u8; 32], 0, 1, 0, 0);

        Self {
            blocks: vec![genesis],
            difficulty_adjustment: DifficultyAdjustment::default(),
            utxo_set: UtxoSet::new(),
        }
    }

    // ============================================================
    // BASIC ACCESSORS
    // ============================================================
    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub fn genesis(&self) -> &Block {
        &self.blocks[0]
    }

    pub fn latest_block(&self) -> &Block {
        self.blocks
            .last()
            .expect("blockchain must contain at least one block")
    }

    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    pub fn blocks_mut(&mut self) -> &mut [Block] {
        &mut self.blocks
    }

    pub fn utxo_set(&self) -> &UtxoSet {
        &self.utxo_set
    }

    // ============================================================
    // MINING SUPPLY
    // ============================================================
    /// Returns only newly minted mining rewards.
    ///
    /// Transaction fees are not new supply.
    /// /// The 100 NIO project reserve is outside mining supply.
    pub fn mining_supply(&self) -> u64 {
        let mut supply = 0u64;
        for block in self.blocks.iter().skip(1) {
            let reward = Economy::block_reward(block.header.height, supply);
            supply = supply.checked_add(reward).expect("mining supply overflow");
        }
        supply
    }

    // ============================================================
    // DIFFICULTY
    // ============================================================
    pub fn expected_next_difficulty(&self) -> Result<u32, String> {
        let previous = self.latest_block();
        let next_height = previous
            .header
            .height
            .checked_add(1)
            .ok_or_else(|| "block height overflow".to_string())?;
        let interval = self.difficulty_adjustment.adjustment_interval;

        if interval == 0 {
            return Err("adjustment interval cannot be zero".to_string());
        }

        if next_height % interval != 0 {
            return Ok(previous.header.difficulty);
        }

        if self.blocks.len() < interval as usize {
            return Ok(previous.header.difficulty);
        }

        let interval_usize = interval as usize;
        let start_index = self.blocks.len().saturating_sub(interval_usize);
        let start_block = &self.blocks[start_index];
        let current_difficulty = previous.header.difficulty as u64;
        let expected = self.difficulty_adjustment.expected_difficulty(
            current_difficulty,
            start_block.header.timestamp,
            previous.header.timestamp,
            interval,
        )?;
        u32::try_from(expected).map_err(|_| "calculated difficulty exceeds u32".to_string())
    }

    // ============================================================
    // ADD BLOCK
    // ============================================================
    pub fn add_block(&mut self, block: Block) -> Result<(), String> {
        let previous = self.latest_block();

        // --------------------------------------------------------
        // HEIGHT
        // --------------------------------------------------------
        let expected_height = previous
            .header
            .height
            .checked_add(1)
            .ok_or_else(|| "block height overflow".to_string())?;
        if block.header.height != expected_height {
            return Err("invalid block height".to_string());
        }

        // --------------------------------------------------------
        // PREVIOUS HASH
        // --------------------------------------------------------
        if block.header.previous_hash != previous.hash() {
            return Err("invalid previous hash".to_string());
        }

        // --------------------------------------------------------
        // TIMESTAMP
        // --------------------------------------------------------
        if block.header.timestamp <= previous.header.timestamp {
            return Err("invalid block timestamp".to_string());
        }

        // --------------------------------------------------------
        // DIFFICULTY
        // --------------------------------------------------------
        let expected_difficulty = self.expected_next_difficulty()?;
        if block.header.difficulty != expected_difficulty {
            return Err("invalid block difficulty".to_string());
        }

        // --------------------------------------------------------
        // CURRENT MINING SUPPLY
        // --------------------------------------------------------
        let current_supply = self.mining_supply();
        if !Economy::is_supply_valid(current_supply) {
            return Err("current mining supply is invalid".to_string());
        }

        // --------------------------------------------------------
        // BASE REWARD
        // --------------------------------------------------------
        let expected_base_reward = Economy::block_reward(block.header.height, current_supply);

        // --------------------------------------------------------
        // MERKLE ROOT
        // --------------------------------------------------------
        if !block.has_valid_merkle_root() {
            return Err("invalid merkle root".to_string());
        }

        // --------------------------------------------------------
        // TRANSACTIONS
        // --------------------------------------------------------
        let mut candidate_utxos = self.utxo_set.clone();
        for transaction in &block.transactions {
            if !transaction.validate_basic() {
                return Err("invalid transaction in block".to_string());
            }
            if !transaction.validate_signatures() {
                return Err("invalid transaction signature".to_string());
            }
            candidate_utxos
                .apply_transaction(transaction)
                .map_err(|error| format!("invalid block transaction: {}", error))?;
        }

        // --------------------------------------------------------
        // FEES
        // --------------------------------------------------------
        let total_fees = block.total_transaction_fees()?;

        // --------------------------------------------------------
        // MINER PAYOUT
        // --------------------------------------------------------
        let expected_payout = expected_base_reward
            .checked_add(total_fees)
            .ok_or_else(|| "miner payout overflow".to_string())?;
        if block.miner_reward != expected_payout {
            return Err(format!(
                "invalid miner payout: expected {}, got {}",
                expected_payout, block.miner_reward
            ));
        }

        // --------------------------------------------------------
        // MINING SUPPLY
        // --------------------------------------------------------
        let new_mining_supply = current_supply
            .checked_add(expected_base_reward)
            .ok_or_else(|| "mining supply overflow".to_string())?;
        if !Economy::is_supply_valid(new_mining_supply) {
            return Err("mining supply exceeds hard cap".to_string());
        }

        // --------------------------------------------------------
        // PROOF OF WORK
        // --------------------------------------------------------
        if !Miner::validate(&block) {
            return Err("invalid proof of work".to_string());
        }

        // --------------------------------------------------------
        // COMMIT STATE
        // --------------------------------------------------------
        self.utxo_set = candidate_utxos;
        self.blocks.push(block);
        Ok(())
    }

    // ============================================================
    // FULL CHAIN VALIDATION
    // ============================================================
    pub fn is_valid(&self) -> bool {
        // --------------------------------------------------------
        // CHAIN MUST NOT BE EMPTY
        // --------------------------------------------------------
        if self.blocks.is_empty() {
            return false;
        }

        // --------------------------------------------------------
        // GENESIS POLICY
        // --------------------------------------------------------
        let genesis_state = GenesisState::new();
        if !genesis_state.is_valid() {
            return false;
        }
        if genesis_state.initial_mining_supply() != 0 {
            return false;
        }
        if genesis_state.mining_cap() != Economy::mining_cap() {
            return false;
        }

        // --------------------------------------------------------
        // GENESIS BLOCK
        // --------------------------------------------------------
        let genesis = &self.blocks[0];
        if genesis.header.height != 0 {
            return false;
        }
        if genesis.header.previous_hash != [0u8; 32] {
            return false;
        }
        if genesis.header.timestamp != 0 {
            return false;
        }
        if genesis.miner_reward != 0 {
            return false;
        }
        if !genesis.transactions.is_empty() {
            return false;
        }
        if genesis.header.merkle_root != [0u8; 32] {
            return false;
        }

        // --------------------------------------------------------
        // REBUILD UTXO STATE
        // --------------------------------------------------------
        let mut state = UtxoSet::new();
        let mut mining_supply = 0u64;

        // --------------------------------------------------------
        // VALIDATE ALL BLOCKS AFTER GENESIS
        // --------------------------------------------------------
        for index in 1..self.blocks.len() {
            let block = &self.blocks[index];
            let previous = &self.blocks[index - 1];

            // ----------------------------------------------------
            // HEIGHT
            // ----------------------------------------------------
            let expected_height = match previous.header.height.checked_add(1) {
                Some(value) => value,
                None => return false,
            };
            if block.header.height != expected_height {
                return false;
            }

            // ----------------------------------------------------
            // PREVIOUS HASH
            // ----------------------------------------------------
            if block.header.previous_hash != previous.hash() {
                return false;
            }

            // ----------------------------------------------------
            // TIMESTAMP
            // ----------------------------------------------------
            if block.header.timestamp <= previous.header.timestamp {
                return false;
            }

            // ----------------------------------------------------
            // DIFFICULTY
            // ----------------------------------------------------
            let expected_difficulty = match self.expected_difficulty_for_index(index) {
                Ok(value) => value,
                Err(_) => return false,
            };
            if block.header.difficulty != expected_difficulty {
                return false;
            }

            // ----------------------------------------------------
            // CURRENT MINING SUPPLY
            // ----------------------------------------------------
            if !Economy::is_supply_valid(mining_supply) {
                return false;
            }

            // ----------------------------------------------------
            // BASE REWARD
            // ----------------------------------------------------
            let expected_base_reward = Economy::block_reward(block.header.height, mining_supply);

            // ----------------------------------------------------
            // MERKLE ROOT
            // ----------------------------------------------------
            if !block.has_valid_merkle_root() {
                return false;
            }

            // ----------------------------------------------------
            // TRANSACTIONS
            // ----------------------------------------------------
            for transaction in &block.transactions {
                if !transaction.validate_basic() {
                    return false;
                }
                if !transaction.validate_signatures() {
                    return false;
                }
                if state.apply_transaction(transaction).is_err() {
                    return false;
                }
            }

            // ----------------------------------------------------
            // FEES
            // ----------------------------------------------------
            let total_fees = match block.total_transaction_fees() {
                Ok(value) => value,
                Err(_) => return false,
            };

            // ----------------------------------------------------
            // MINER PAYOUT
            // ----------------------------------------------------
            let expected_payout = match expected_base_reward.checked_add(total_fees) {
                Some(value) => value,
                None => return false,
            };
            if block.miner_reward != expected_payout {
                return false;
            }

            // ----------------------------------------------------
            // MINING SUPPLY
            // ----------------------------------------------------
            mining_supply = match mining_supply.checked_add(expected_base_reward) {
                Some(value) => value,
                None => return false,
            };
            if !Economy::is_supply_valid(mining_supply) {
                return false;
            }

            // ----------------------------------------------------
            // PROOF OF WORK
            // ----------------------------------------------------
            if !Miner::validate(block) {
                return false;
            }
        }

        // --------------------------------------------------------
        // FINAL MINING CAP CHECK
        // --------------------------------------------------------
        if mining_supply > Economy::mining_cap() {
            return false;
        }

        // --------------------------------------------------------
        // NOTE
        // --------------------------------------------------------
        // // The reconstructed UTXO state is intentionally kept
        // local for now. Deterministic stored-state comparison
        // will be added as a dedicated security step.
        // --------------------------------------------------------
        let _ = state;
        true
    }

    // ============================================================
    // HISTORICAL DIFFICULTY
    // ============================================================
    fn expected_difficulty_for_index(&self, index: usize) -> Result<u32, String> {
        if index == 0 {
            return Ok(self.blocks[0].header.difficulty);
        }

        let block = &self.blocks[index];
        let previous = &self.blocks[index - 1];
        let next_height = block.header.height;
        let current_difficulty = previous.header.difficulty as u64;
        let interval = self.difficulty_adjustment.adjustment_interval;

        if interval == 0 {
            return Err("adjustment interval cannot be zero".to_string());
        }

        if !next_height.is_multiple_of(interval) {
            return Ok(previous.header.difficulty);
        }

        let interval_usize = interval as usize;
        if index < interval_usize {
            return Ok(previous.header.difficulty);
        }

        let start_index = index - interval_usize;
        let start_block = &self.blocks[start_index];
        let expected = self.difficulty_adjustment.expected_difficulty(
            current_difficulty,
            start_block.header.timestamp,
            previous.header.timestamp,
            interval,
        )?;
        u32::try_from(expected).map_err(|_| "calculated difficulty exceeds u32".to_string())
    }
}

// ================================================================
// DEFAULT
// ================================================================
impl Default for Blockchain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod stress_tests {
    use super::*;

    #[test]
    fn stress_long_chain_100_blocks() {
        let mut chain = Blockchain::new();

        for height in 1..=100u64 {
            let previous_hash = chain.latest_block().hash();

            let supply = chain.mining_supply();

            let reward = Economy::block_reward(height, supply);

            let difficulty = chain
                .expected_next_difficulty()
                .expect("difficulty calculation must succeed");

            let mut block =
                Block::new(1, height, previous_hash, height * 60, difficulty, 0, reward);

            Miner::mine(&mut block).expect("stress block must be mineable");

            chain
                .add_block(block)
                .expect("valid stress block must be accepted");

            assert_eq!(chain.len(), height as usize + 1);
        }

        assert_eq!(chain.len(), 101);
        assert!(chain.is_valid());
    }

    #[test]
    fn stress_chain_state_remains_valid_after_each_block() {
        let mut chain = Blockchain::new();

        for height in 1..=50u64 {
            let previous_hash = chain.latest_block().hash();
            let supply = chain.mining_supply();
            let reward = Economy::block_reward(height, supply);

            let difficulty = chain
                .expected_next_difficulty()
                .expect("difficulty calculation must succeed");

            let mut block =
                Block::new(1, height, previous_hash, height * 60, difficulty, 0, reward);

            Miner::mine(&mut block).expect("block must be mineable");

            chain.add_block(block).expect("block must be accepted");

            assert!(
                chain.is_valid(),
                "chain became invalid at height {}",
                height
            );
        }
    }

    #[test]
    fn stress_repeated_chain_validation() {
        let mut chain = Blockchain::new();

        for height in 1..=30u64 {
            let previous_hash = chain.latest_block().hash();
            let supply = chain.mining_supply();
            let reward = Economy::block_reward(height, supply);
            let difficulty = chain
                .expected_next_difficulty()
                .expect("difficulty calculation must succeed");

            let mut block =
                Block::new(1, height, previous_hash, height * 60, difficulty, 0, reward);

            Miner::mine(&mut block).expect("block must be mineable");

            chain.add_block(block).expect("block must be accepted");
        }

        for _ in 0..100 {
            assert!(chain.is_valid());
        }
    }
}
