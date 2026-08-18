use crate::block::Block;
use crate::economy::Economy;
use crate::mining::Miner;
use crate::utxo::UtxoSet;

pub struct ConsensusValidator;

impl ConsensusValidator {
    // ============================================================
    // COMPLETE BLOCK VALIDATION
    // ============================================================

    pub fn validate_block(
        block: &Block,
        expected_previous_hash: [u8; 32],
        expected_height: u64,
        current_mining_supply: u64,
        utxos: &UtxoSet,
    ) -> Result<u64, String> {
        // --------------------------------------------------------
        // 1. SUPPLY STATE
        // --------------------------------------------------------

        if !Economy::is_supply_valid(current_mining_supply) {
            return Err("current mining supply exceeds hard cap".to_string());
        }

        // --------------------------------------------------------
        // 2. HEIGHT
        // --------------------------------------------------------

        if block.header.height != expected_height {
            return Err(format!(
                "invalid block height: expected {}, got {}",
                expected_height, block.header.height
            ));
        }

        // --------------------------------------------------------
        // 3. PREVIOUS HASH
        // --------------------------------------------------------

        if block.header.previous_hash != expected_previous_hash {
            return Err("invalid previous block hash".to_string());
        }

        // --------------------------------------------------------
        // 4. MERKLE ROOT
        // --------------------------------------------------------

        if !block.transactions.is_empty() && !block.has_valid_merkle_root() {
            return Err("invalid merkle root".to_string());
        }

        // --------------------------------------------------------
        // 5. PROOF OF WORK
        // --------------------------------------------------------

        if block.header.difficulty == 0 {
            return Err("block difficulty cannot be zero".to_string());
        }

        if !Miner::validate(block) {
            return Err("invalid proof of work".to_string());
        }

        // --------------------------------------------------------
        // 6. BASE MINING REWARD
        // --------------------------------------------------------

        let expected_base_reward =
            Economy::block_reward(block.header.height, current_mining_supply);

        // --------------------------------------------------------
        // 7. TRANSACTION STATE TRANSITION
        // --------------------------------------------------------
        //
        // Never mutate the caller's UTXO state during validation.
        // All transaction effects are applied to a candidate copy.
        //

        let mut candidate_utxos = utxos.clone();
        let mut total_fees = 0u64;

        for transaction in &block.transactions {
            // Coinbase transactions must not appear in the normal
            // transaction list because miner reward is represented
            // separately by block.miner_reward.
            if transaction.is_coinbase() {
                return Err("coinbase transaction cannot appear in block transactions".to_string());
            }

            let fee = candidate_utxos
                .apply_transaction(transaction)
                .map_err(|error| format!("invalid transaction in block: {}", error))?;

            total_fees = total_fees
                .checked_add(fee)
                .ok_or_else(|| "block fee total overflow".to_string())?;
        }

        // --------------------------------------------------------
        // 8. MINER PAYOUT
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
        // 9. REWARD SAFETY
        // --------------------------------------------------------

        if block.miner_reward < expected_base_reward {
            return Err("miner reward is below required base reward".to_string());
        }

        // --------------------------------------------------------
        // 10. MINING SUPPLY TRANSITION
        // --------------------------------------------------------

        let next_mining_supply = current_mining_supply
            .checked_add(expected_base_reward)
            .ok_or_else(|| "mining supply overflow".to_string())?;

        if !Economy::is_supply_valid(next_mining_supply) {
            return Err("resulting mining supply exceeds hard cap".to_string());
        }

        // --------------------------------------------------------
        // 11. CANDIDATE STATE INTEGRITY
        // --------------------------------------------------------
        //
        // The candidate state must itself have a deterministic
        // canonical hash. This also makes the validation path
        // independent from HashMap iteration order.
        //

        let candidate_hash = candidate_utxos.state_hash();

        if candidate_hash == [0u8; 32] && !candidate_utxos.is_empty() {
            return Err("invalid candidate UTXO state hash".to_string());
        }

        // --------------------------------------------------------
        // 12. FINAL CONSENSUS ACCEPTANCE
        // --------------------------------------------------------

        Ok(total_fees)
    }

    // ============================================================
    // HEADER VALIDATION
    // ============================================================

    pub fn validate_header(
        block: &Block,
        expected_previous_hash: [u8; 32],
        expected_height: u64,
    ) -> Result<(), String> {
        // --------------------------------------------------------
        // 1. HEIGHT
        // --------------------------------------------------------

        if block.header.height != expected_height {
            return Err(format!(
                "invalid block height: expected {}, got {}",
                expected_height, block.header.height
            ));
        }

        // --------------------------------------------------------
        // 2. PREVIOUS HASH
        // --------------------------------------------------------

        if block.header.previous_hash != expected_previous_hash {
            return Err("invalid previous hash".to_string());
        }

        // --------------------------------------------------------
        // 3. DIFFICULTY
        // --------------------------------------------------------

        if block.header.difficulty == 0 {
            return Err("invalid zero difficulty".to_string());
        }

        // --------------------------------------------------------
        // 4. MERKLE ROOT
        // --------------------------------------------------------

        if !block.transactions.is_empty() && !block.has_valid_merkle_root() {
            return Err("invalid merkle root".to_string());
        }

        Ok(())
    }
}

// ================================================================
// TESTS
// ================================================================

#[cfg(test)]
mod tests {
    use super::*;

    use crate::block::Block;
    use crate::economy::INITIAL_REWARD_UNITS;
    use crate::transaction::{Transaction, TransactionInput, TransactionOutput};

    fn empty_utxos() -> UtxoSet {
        UtxoSet::new()
    }

    fn tx(value: u8) -> Transaction {
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
            0,
        )
    }

    // ------------------------------------------------------------
    // VALID HEADER
    // ------------------------------------------------------------

    #[test]
    fn valid_header_is_accepted() {
        let previous_hash = [7u8; 32];

        let block = Block::new(1, 1, previous_hash, 1_000, 1, 0, INITIAL_REWARD_UNITS);

        assert!(ConsensusValidator::validate_header(&block, previous_hash, 1).is_ok());
    }

    // ------------------------------------------------------------
    // WRONG HEIGHT
    // ------------------------------------------------------------

    #[test]
    fn wrong_height_is_rejected() {
        let previous_hash = [7u8; 32];

        let block = Block::new(1, 5, previous_hash, 1_000, 1, 0, INITIAL_REWARD_UNITS);

        assert!(
            ConsensusValidator::validate_block(&block, previous_hash, 1, 0, &empty_utxos(),)
                .is_err()
        );
    }

    // ------------------------------------------------------------
    // WRONG PREVIOUS HASH
    // ------------------------------------------------------------

    #[test]
    fn wrong_previous_hash_is_rejected() {
        let block = Block::new(1, 1, [8u8; 32], 1_000, 1, 0, INITIAL_REWARD_UNITS);

        assert!(
            ConsensusValidator::validate_block(&block, [7u8; 32], 1, 0, &empty_utxos(),).is_err()
        );
    }

    // ------------------------------------------------------------
    // VALID EMPTY BLOCK
    // ------------------------------------------------------------

    #[test]
    fn valid_empty_block_is_accepted() {
        let previous_hash = [7u8; 32];

        let mut block = Block::new(1, 1, previous_hash, 1_000, 1, 0, INITIAL_REWARD_UNITS);

        // Mine the test block so its proof-of-work is actually valid.
        crate::mining::Miner::mine(&mut block).expect("test block should be mineable");

        let result =
            ConsensusValidator::validate_block(&block, previous_hash, 1, 0, &empty_utxos());

        assert!(result.is_ok(), "CONSENSUS ERROR: {:?}", result);
        assert_eq!(result.unwrap(), 0);
    }

    // ------------------------------------------------------------
    // EXCESSIVE REWARD
    // ------------------------------------------------------------

    #[test]
    fn excessive_reward_is_rejected() {
        let previous_hash = [7u8; 32];

        let block = Block::new(1, 1, previous_hash, 1_000, 1, 0, INITIAL_REWARD_UNITS + 1);

        assert!(
            ConsensusValidator::validate_block(&block, previous_hash, 1, 0, &empty_utxos(),)
                .is_err()
        );
    }

    // ------------------------------------------------------------
    // INVALID MERKLE ROOT
    // ------------------------------------------------------------

    #[test]
    fn invalid_merkle_root_is_rejected() {
        let previous_hash = [7u8; 32];

        let mut block = Block::new(1, 1, previous_hash, 1_000, 1, 0, INITIAL_REWARD_UNITS);

        block.header.merkle_root = [9u8; 32];

        assert!(
            ConsensusValidator::validate_block(&block, previous_hash, 1, 0, &empty_utxos(),)
                .is_err()
        );
    }

    // ------------------------------------------------------------
    // SUPPLY CAP
    // ------------------------------------------------------------

    #[test]
    fn supply_above_cap_is_rejected() {
        let previous_hash = [7u8; 32];

        let block = Block::new(1, 1, previous_hash, 1_000, 1, 0, 0);

        let result = ConsensusValidator::validate_block(
            &block,
            previous_hash,
            1,
            Economy::mining_cap() + 1,
            &empty_utxos(),
        );

        assert!(result.is_err());
    }

    // ------------------------------------------------------------
    // TRANSACTION VALIDATION
    // ------------------------------------------------------------

    #[test]
    fn invalid_transaction_is_rejected() {
        let previous_hash = [7u8; 32];

        let transaction = tx(1);

        let block = Block::new_with_transactions(
            1,
            1,
            previous_hash,
            1_000,
            1,
            0,
            INITIAL_REWARD_UNITS,
            vec![transaction],
        );

        assert!(
            ConsensusValidator::validate_block(&block, previous_hash, 1, 0, &empty_utxos(),)
                .is_err()
        );
    }

    // ------------------------------------------------------------
    // 8.4 BLOCK-LEVEL DOUBLE SPEND
    // ------------------------------------------------------------

    #[test]
    fn double_spend_inside_same_block_is_rejected() {
        use secp256k1::{PublicKey, Secp256k1, SecretKey};

        let previous_hash = [7u8; 32];

        let secret_key = SecretKey::from_slice(&[1u8; 32]).expect("valid secret key");

        let secp = Secp256k1::new();

        let public_key = PublicKey::from_secret_key(&secp, &secret_key)
            .serialize()
            .to_vec();

        let previous_output = [1u8; 32];

        let mut utxos = UtxoSet::new();

        utxos
            .insert(crate::utxo::Utxo {
                id: crate::utxo::UtxoId {
                    transaction_id: previous_output,
                    output_index: 0,
                },
                amount: 1_000,
                recipient: public_key.clone(),
            })
            .expect("UTXO insert");

        let mut tx1 = Transaction::new(
            1,
            vec![TransactionInput {
                previous_output,
                output_index: 0,
                public_key: Vec::new(),
                signature: Vec::new(),
            }],
            vec![TransactionOutput {
                amount: 900,
                recipient: public_key.clone(),
            }],
            100,
        );

        tx1.sign_input(0, &secret_key).expect("tx1 should sign");

        let mut tx2 = Transaction::new(
            1,
            vec![TransactionInput {
                previous_output,
                output_index: 0,
                public_key: Vec::new(),
                signature: Vec::new(),
            }],
            vec![TransactionOutput {
                amount: 800,
                recipient: public_key,
            }],
            200,
        );

        tx2.sign_input(0, &secret_key).expect("tx2 should sign");

        let mut block = Block::new_with_transactions(
            1,
            1,
            previous_hash,
            1_000,
            1,
            0,
            INITIAL_REWARD_UNITS + 100 + 200,
            vec![tx1, tx2],
        );

        crate::mining::Miner::mine(&mut block).expect("block should be mineable");

        let result = ConsensusValidator::validate_block(&block, previous_hash, 1, 0, &utxos);

        assert!(
            result.is_err(),
            "double spend inside block must be rejected"
        );
    }

    // ------------------------------------------------------------
    // 8.4 INVALID TOTAL FEE
    // ------------------------------------------------------------

    #[test]
    fn incorrect_total_miner_reward_is_rejected() {
        use secp256k1::{PublicKey, Secp256k1, SecretKey};

        let previous_hash = [7u8; 32];

        let secret_key = SecretKey::from_slice(&[1u8; 32]).expect("valid secret key");

        let secp = Secp256k1::new();

        let public_key = PublicKey::from_secret_key(&secp, &secret_key)
            .serialize()
            .to_vec();

        let previous_output = [2u8; 32];

        let mut utxos = UtxoSet::new();

        utxos
            .insert(crate::utxo::Utxo {
                id: crate::utxo::UtxoId {
                    transaction_id: previous_output,
                    output_index: 0,
                },
                amount: 1_000,
                recipient: public_key.clone(),
            })
            .expect("UTXO insert");

        let mut transaction = Transaction::new(
            1,
            vec![TransactionInput {
                previous_output,
                output_index: 0,
                public_key: Vec::new(),
                signature: Vec::new(),
            }],
            vec![TransactionOutput {
                amount: 900,
                recipient: public_key,
            }],
            100,
        );

        transaction
            .sign_input(0, &secret_key)
            .expect("transaction should sign");

        let mut block = Block::new_with_transactions(
            1,
            1,
            previous_hash,
            1_000,
            1,
            0,
            INITIAL_REWARD_UNITS,
            vec![transaction],
        );

        crate::mining::Miner::mine(&mut block).expect("block should be mineable");

        let result = ConsensusValidator::validate_block(&block, previous_hash, 1, 0, &utxos);

        assert!(
            result.is_err(),
            "miner reward without transaction fee must be rejected"
        );
    }
    // ------------------------------------------------------------
    // 8.5 ATOMIC BLOCK ROLLBACK
    // ------------------------------------------------------------

    #[test]
    fn failed_block_does_not_modify_original_utxo_state() {
        use secp256k1::{PublicKey, Secp256k1, SecretKey};

        let previous_hash = [7u8; 32];

        let secret_key = SecretKey::from_slice(&[1u8; 32]).expect("valid secret key");

        let secp = Secp256k1::new();

        let public_key = PublicKey::from_secret_key(&secp, &secret_key)
            .serialize()
            .to_vec();

        let previous_output = [3u8; 32];

        let mut utxos = UtxoSet::new();

        utxos
            .insert(crate::utxo::Utxo {
                id: crate::utxo::UtxoId {
                    transaction_id: previous_output,
                    output_index: 0,
                },
                amount: 1_000,
                recipient: public_key.clone(),
            })
            .expect("UTXO insert");

        let original_hash = utxos.state_hash();
        let original_len = utxos.len();

        let mut valid_tx = Transaction::new(
            1,
            vec![TransactionInput {
                previous_output,
                output_index: 0,
                public_key: Vec::new(),
                signature: Vec::new(),
            }],
            vec![TransactionOutput {
                amount: 900,
                recipient: public_key.clone(),
            }],
            100,
        );

        valid_tx
            .sign_input(0, &secret_key)
            .expect("valid transaction should sign");

        let invalid_tx = tx(99);

        let mut block = Block::new_with_transactions(
            1,
            1,
            previous_hash,
            1_000,
            1,
            0,
            INITIAL_REWARD_UNITS + 100,
            vec![valid_tx, invalid_tx],
        );

        crate::mining::Miner::mine(&mut block).expect("block should be mineable");

        let result = ConsensusValidator::validate_block(&block, previous_hash, 1, 0, &utxos);

        assert!(result.is_err());

        assert_eq!(
            utxos.state_hash(),
            original_hash,
            "failed block changed original UTXO state"
        );

        assert_eq!(
            utxos.len(),
            original_len,
            "failed block changed original UTXO count"
        );
    }

    // ------------------------------------------------------------
    // 8.5 FEE ACCOUNTING
    // ------------------------------------------------------------

    #[test]
    fn miner_reward_includes_all_transaction_fees() {
        use secp256k1::{PublicKey, Secp256k1, SecretKey};

        let previous_hash = [7u8; 32];

        let secret_key = SecretKey::from_slice(&[1u8; 32]).expect("valid secret key");

        let secp = Secp256k1::new();

        let public_key = PublicKey::from_secret_key(&secp, &secret_key)
            .serialize()
            .to_vec();

        let previous_output_1 = [4u8; 32];
        let previous_output_2 = [5u8; 32];

        let mut utxos = UtxoSet::new();

        utxos
            .insert(crate::utxo::Utxo {
                id: crate::utxo::UtxoId {
                    transaction_id: previous_output_1,
                    output_index: 0,
                },
                amount: 1_000,
                recipient: public_key.clone(),
            })
            .expect("UTXO 1 insert");

        utxos
            .insert(crate::utxo::Utxo {
                id: crate::utxo::UtxoId {
                    transaction_id: previous_output_2,
                    output_index: 0,
                },
                amount: 2_000,
                recipient: public_key.clone(),
            })
            .expect("UTXO 2 insert");

        let mut tx1 = Transaction::new(
            1,
            vec![TransactionInput {
                previous_output: previous_output_1,
                output_index: 0,
                public_key: Vec::new(),
                signature: Vec::new(),
            }],
            vec![TransactionOutput {
                amount: 900,
                recipient: public_key.clone(),
            }],
            100,
        );

        tx1.sign_input(0, &secret_key).expect("tx1 should sign");

        let mut tx2 = Transaction::new(
            1,
            vec![TransactionInput {
                previous_output: previous_output_2,
                output_index: 0,
                public_key: Vec::new(),
                signature: Vec::new(),
            }],
            vec![TransactionOutput {
                amount: 1_800,
                recipient: public_key,
            }],
            200,
        );

        tx2.sign_input(0, &secret_key).expect("tx2 should sign");

        let expected_reward = INITIAL_REWARD_UNITS + 100 + 200;

        let mut block = Block::new_with_transactions(
            1,
            1,
            previous_hash,
            1_000,
            1,
            0,
            expected_reward,
            vec![tx1, tx2],
        );

        crate::mining::Miner::mine(&mut block).expect("block should be mineable");

        let result = ConsensusValidator::validate_block(&block, previous_hash, 1, 0, &utxos);

        assert!(result.is_ok(), "CONSENSUS ERROR: {:?}", result);
        assert_eq!(result.unwrap(), 300);
    }
    // ------------------------------------------------------------
    // STATE SAFETY
    // ------------------------------------------------------------

    #[test]
    fn validation_does_not_modify_utxo_state() {
        let previous_hash = [7u8; 32];

        let utxos = UtxoSet::new();
        let original_hash = utxos.state_hash();

        let block = Block::new(1, 1, previous_hash, 1_000, 1, 0, INITIAL_REWARD_UNITS);

        let _ = ConsensusValidator::validate_block(&block, previous_hash, 1, 0, &utxos);

        assert_eq!(utxos.state_hash(), original_hash);
    }

    // ------------------------------------------------------------
    // VALID TRANSACTION BLOCK
    // ------------------------------------------------------------

    #[test]
    fn valid_transaction_block_is_accepted_and_fee_is_returned() {
        use secp256k1::{PublicKey, Secp256k1, SecretKey};

        let previous_hash = [7u8; 32];

        let secret_key = SecretKey::from_slice(&[1u8; 32]).expect("valid secret key");

        let secp = Secp256k1::new();

        let public_key = PublicKey::from_secret_key(&secp, &secret_key)
            .serialize()
            .to_vec();

        let mut utxos = UtxoSet::new();

        let previous_output = [1u8; 32];

        utxos
            .insert(crate::utxo::Utxo {
                id: crate::utxo::UtxoId {
                    transaction_id: previous_output,
                    output_index: 0,
                },
                amount: 1_000,
                recipient: public_key.clone(),
            })
            .expect("UTXO insert should succeed");

        let mut transaction = Transaction::new(
            1,
            vec![TransactionInput {
                previous_output,
                output_index: 0,
                public_key: Vec::new(),
                signature: Vec::new(),
            }],
            vec![TransactionOutput {
                amount: 900,
                recipient: public_key,
            }],
            100,
        );

        transaction
            .sign_input(0, &secret_key)
            .expect("transaction should sign");

        let mut block = Block::new_with_transactions(
            1,
            1,
            previous_hash,
            1_000,
            1,
            0,
            INITIAL_REWARD_UNITS + 100,
            vec![transaction],
        );

        crate::mining::Miner::mine(&mut block).expect("block should be mineable");

        let result = ConsensusValidator::validate_block(&block, previous_hash, 1, 0, &utxos);

        assert!(result.is_ok(), "CONSENSUS ERROR: {:?}", result);
        assert_eq!(result.unwrap(), 100);
    }
    // ------------------------------------------------------------
    // ZERO DIFFICULTY
    // ------------------------------------------------------------

    #[test]
    fn zero_difficulty_is_rejected() {
        let previous_hash = [7u8; 32];

        let block = Block::new(1, 1, previous_hash, 1_000, 0, 0, INITIAL_REWARD_UNITS);

        assert!(ConsensusValidator::validate_header(&block, previous_hash, 1).is_err());
    }
}
