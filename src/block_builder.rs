use crate::block::Block;
use crate::chain::Blockchain;
use crate::economy::Economy;
use crate::mempool::Mempool;
use crate::mining::Miner;
use crate::transaction::{Transaction, TransactionId};
use crate::utxo::UtxoSet;

/// Responsible for building block templates and preparing them for mining.
pub struct BlockBuilder;

impl BlockBuilder {
    // ============================================================
    // TRANSACTION SELECTION
    // ============================================================

    /// Select transactions by highest fee first.
    pub fn select_transactions(mempool: &Mempool, max_transactions: usize) -> Vec<Transaction> {
        if max_transactions == 0 {
            return Vec::new();
        }

        mempool
            .transactions_by_fee_desc()
            .into_iter()
            .take(max_transactions)
            .cloned()
            .collect()
    }

    // ============================================================
    // TRANSACTION VALIDATION
    // ============================================================

    /// Validate selected transactions against a cloned UTXO set.
    ///
    /// The real blockchain UTXO state is not modified.
    pub fn validate_selected_transactions(
        transactions: &[Transaction],
        utxos: &UtxoSet,
    ) -> Result<u64, String> {
        let mut candidate_utxos = utxos.clone();
        let mut total_fees = 0u64;

        for transaction in transactions {
            if !transaction.validate_basic() {
                return Err("invalid transaction".to_string());
            }

            if !transaction.validate_signatures() {
                return Err("invalid transaction signature".to_string());
            }

            let fee = candidate_utxos
                .apply_transaction(transaction)
                .map_err(|error| format!("invalid transaction UTXO state: {}", error))?;

            total_fees = total_fees
                .checked_add(fee)
                .ok_or_else(|| "total transaction fees overflow".to_string())?;
        }

        Ok(total_fees)
    }

    // ============================================================
    // BLOCK TEMPLATE VALIDATION
    // ============================================================

    pub fn validate_block_template(block: &Block) -> Result<(), String> {
        if !block.has_valid_merkle_root() {
            return Err("invalid merkle root in block template".to_string());
        }

        if block.header.difficulty == 0 {
            return Err("block difficulty must be greater than zero".to_string());
        }

        if block.header.difficulty > 32 {
            return Err("block difficulty cannot exceed 32".to_string());
        }

        Ok(())
    }

    // ============================================================
    // BUILD TEMPLATE
    // ============================================================

    pub fn build_template(
        blockchain: &Blockchain,
        mempool: &Mempool,
        max_transactions: usize,
        timestamp: u64,
    ) -> Result<Block, String> {
        let previous = blockchain.latest_block();

        // --------------------------------------------------------
        // HEIGHT
        // --------------------------------------------------------

        let height = previous
            .header
            .height
            .checked_add(1)
            .ok_or_else(|| "block height overflow".to_string())?;

        // --------------------------------------------------------
        // TIMESTAMP
        // --------------------------------------------------------

        if timestamp <= previous.header.timestamp {
            return Err(
                "new block timestamp must be greater than previous block timestamp".to_string(),
            );
        }

        // --------------------------------------------------------
        // DIFFICULTY
        // --------------------------------------------------------

        let difficulty = blockchain.expected_next_difficulty()?;

        if difficulty == 0 {
            return Err("calculated difficulty is zero".to_string());
        }

        if difficulty > 32 {
            return Err("calculated difficulty exceeds SHA-256 hash size".to_string());
        }

        // --------------------------------------------------------
        // TRANSACTION SELECTION
        // --------------------------------------------------------

        let transactions = Self::select_transactions(mempool, max_transactions);

        // --------------------------------------------------------
        // UTXO VALIDATION + FEES
        // --------------------------------------------------------

        let total_fees =
            Self::validate_selected_transactions(&transactions, blockchain.utxo_set())?;

        // --------------------------------------------------------
        // CURRENT MINING SUPPLY
        // --------------------------------------------------------

        let current_supply = blockchain.mining_supply();

        if !Economy::is_supply_valid(current_supply) {
            return Err("current mining supply is invalid".to_string());
        }

        // --------------------------------------------------------
        // BASE REWARD
        // --------------------------------------------------------

        let base_reward = Economy::block_reward(height, current_supply);

        // --------------------------------------------------------
        // MINER PAYOUT
        // --------------------------------------------------------

        let miner_payout = base_reward
            .checked_add(total_fees)
            .ok_or_else(|| "miner payout overflow".to_string())?;

        // --------------------------------------------------------
        // PREVIOUS HASH
        // --------------------------------------------------------

        let previous_hash = previous.hash();

        // --------------------------------------------------------
        // BUILD BLOCK
        // --------------------------------------------------------

        let block = Block::new_with_transactions(
            1,
            height,
            previous_hash,
            timestamp,
            difficulty,
            0,
            miner_payout,
            transactions,
        );

        // --------------------------------------------------------
        // TEMPLATE VALIDATION
        // --------------------------------------------------------

        Self::validate_block_template(&block)?;

        Ok(block)
    }

    // ============================================================
    // BUILD + MINE
    // ============================================================

    pub fn build_and_mine(
        blockchain: &Blockchain,
        mempool: &Mempool,
        max_transactions: usize,
        timestamp: u64,
    ) -> Result<Block, String> {
        let mut block = Self::build_template(blockchain, mempool, max_transactions, timestamp)?;

        Miner::mine(&mut block)?;

        if !Miner::validate(&block) {
            return Err("mined block failed proof-of-work validation".to_string());
        }

        Ok(block)
    }

    // ============================================================
    // BUILD + MINE + COMMIT
    // ============================================================

    pub fn build_mine_and_commit(
        blockchain: &mut Blockchain,
        mempool: &mut Mempool,
        max_transactions: usize,
        timestamp: u64,
    ) -> Result<Block, String> {
        // --------------------------------------------------------
        // BUILD + MINE
        // --------------------------------------------------------

        let block = Self::build_and_mine(blockchain, mempool, max_transactions, timestamp)?;

        // --------------------------------------------------------
        // SAVE TRANSACTION IDS
        // --------------------------------------------------------

        let transaction_ids: Vec<TransactionId> =
            block.transactions.iter().map(|tx| tx.id()).collect();

        // --------------------------------------------------------
        // COMMIT BLOCK
        // --------------------------------------------------------

        blockchain.add_block(block.clone())?;

        // --------------------------------------------------------
        // REMOVE CONFIRMED TRANSACTIONS
        // --------------------------------------------------------

        mempool.remove_many(&transaction_ids);

        Ok(block)
    }
}

// ================================================================
// TESTS
// ================================================================

#[cfg(test)]
mod tests {
    use super::*;

    use crate::economy::INITIAL_REWARD_UNITS;
    use crate::transaction::{TransactionInput, TransactionOutput};
    use crate::utxo::{Utxo, UtxoId};

    use secp256k1::{PublicKey, Secp256k1, SecretKey};

    // ============================================================
    // TEST HELPERS
    // ============================================================

    fn secret_key() -> SecretKey {
        SecretKey::from_slice(&[1u8; 32]).expect("valid secret key")
    }

    fn public_key() -> Vec<u8> {
        let secp = Secp256k1::new();

        let public = PublicKey::from_secret_key(&secp, &secret_key());

        public.serialize().to_vec()
    }

    fn input(value: u8) -> TransactionInput {
        TransactionInput {
            previous_output: [value; 32],
            output_index: 0,
            public_key: Vec::new(),
            signature: Vec::new(),
        }
    }

    fn output(amount: u64) -> TransactionOutput {
        TransactionOutput {
            amount,
            recipient: public_key(),
        }
    }

    fn signed_transaction(value: u8, output_amount: u64, fee: u64) -> Transaction {
        let mut tx = Transaction::new(1, vec![input(value)], vec![output(output_amount)], fee);

        tx.sign_input(0, &secret_key())
            .expect("transaction should be signed");

        tx
    }

    fn utxo(value: u8, amount: u64) -> Utxo {
        Utxo {
            id: UtxoId {
                transaction_id: [value; 32],
                output_index: 0,
            },
            amount,
            recipient: public_key(),
        }
    }

    // ============================================================
    // SELECTION TESTS
    // ============================================================

    #[test]
    fn selects_highest_fee_first() {
        let mut mempool = Mempool::new();

        mempool.add(signed_transaction(1, 90, 10)).expect("tx1");

        mempool.add(signed_transaction(2, 70, 30)).expect("tx2");

        mempool.add(signed_transaction(3, 80, 20)).expect("tx3");

        let selected = BlockBuilder::select_transactions(&mempool, 3);

        assert_eq!(selected.len(), 3);
        assert_eq!(selected[0].fee, 30);
        assert_eq!(selected[1].fee, 20);
        assert_eq!(selected[2].fee, 10);
    }

    #[test]
    fn selection_limit_is_respected() {
        let mut mempool = Mempool::new();

        mempool.add(signed_transaction(1, 90, 10)).expect("tx1");

        mempool.add(signed_transaction(2, 70, 30)).expect("tx2");

        mempool.add(signed_transaction(3, 80, 20)).expect("tx3");

        let selected = BlockBuilder::select_transactions(&mempool, 2);

        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].fee, 30);
        assert_eq!(selected[1].fee, 20);
    }

    #[test]
    fn zero_selection_limit_returns_empty() {
        let mempool = Mempool::new();

        let selected = BlockBuilder::select_transactions(&mempool, 0);

        assert!(selected.is_empty());
    }

    // ============================================================
    // UTXO VALIDATION TESTS
    // ============================================================

    #[test]
    fn selected_transactions_are_validated() {
        let mut utxos = UtxoSet::new();

        utxos.insert(utxo(1, 100)).expect("utxo insertion");

        let tx = signed_transaction(1, 90, 10);

        let fee = BlockBuilder::validate_selected_transactions(&[tx], &utxos)
            .expect("transaction should be valid");

        assert_eq!(fee, 10);
    }

    #[test]
    fn invalid_transaction_is_rejected() {
        let utxos = UtxoSet::new();

        let tx = signed_transaction(1, 90, 10);

        assert!(BlockBuilder::validate_selected_transactions(&[tx], &utxos,).is_err());
    }

    #[test]
    fn missing_utxo_is_rejected() {
        let utxos = UtxoSet::new();

        let tx = signed_transaction(99, 90, 10);

        assert!(BlockBuilder::validate_selected_transactions(&[tx], &utxos,).is_err());
    }

    // ============================================================
    // TEMPLATE TESTS
    // ============================================================

    #[test]
    fn empty_mempool_can_create_template() {
        let blockchain = Blockchain::new();
        let mempool = Mempool::new();

        let block = BlockBuilder::build_template(&blockchain, &mempool, 100, 1_000)
            .expect("template should build");

        assert_eq!(block.header.height, 1);

        assert_eq!(block.header.previous_hash, blockchain.latest_block().hash());

        assert!(block.transactions.is_empty());

        assert_eq!(block.miner_reward, INITIAL_REWARD_UNITS);

        assert!(block.has_valid_merkle_root());
    }

    #[test]
    fn timestamp_must_increase() {
        let blockchain = Blockchain::new();
        let mempool = Mempool::new();

        let result = BlockBuilder::build_template(&blockchain, &mempool, 10, 0);

        assert!(result.is_err());
    }

    #[test]
    fn template_has_valid_merkle_root() {
        let blockchain = Blockchain::new();
        let mempool = Mempool::new();

        let block = BlockBuilder::build_template(&blockchain, &mempool, 10, 1_000)
            .expect("template should build");

        assert!(block.has_valid_merkle_root());
    }

    // ============================================================
    // POW TEST
    // ============================================================

    #[test]
    fn build_and_mine_produces_valid_pow() {
        let blockchain = Blockchain::new();
        let mempool = Mempool::new();

        let block = BlockBuilder::build_and_mine(&blockchain, &mempool, 10, 1_000)
            .expect("mining should succeed");

        assert!(Miner::validate(&block));
    }
}
