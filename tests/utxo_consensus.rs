use nio_blockchain::block::Block;
use nio_blockchain::chain::Blockchain;
use nio_blockchain::transaction::{Transaction, TransactionInput, TransactionOutput};
use nio_blockchain::utxo::{Utxo, UtxoId, UtxoSet};

#[test]
fn genesis_block_has_zero_mining_supply() {
    let blockchain = Blockchain::new();

    assert_eq!(blockchain.mining_supply(), 0);
    assert_eq!(blockchain.genesis().header.height, 0);
}

#[test]
fn blockchain_starts_with_empty_utxo_set() {
    let utxo_set = UtxoSet::new();

    assert!(utxo_set.is_empty());
    assert_eq!(utxo_set.len(), 0);
}

#[test]
fn blockchain_remains_valid_after_creation() {
    let blockchain = Blockchain::new();

    assert!(blockchain.is_valid());
}

#[test]
fn invalid_block_does_not_change_chain_length() {
    let mut blockchain = Blockchain::new();

    let original_len = blockchain.len();

    let invalid_block = Block::new(1, 99, [0u8; 32], 1, 1, 0, 0);

    assert!(blockchain.add_block(invalid_block).is_err());
    assert_eq!(blockchain.len(), original_len);
}

#[test]
fn invalid_block_does_not_modify_utxo_state() {
    let blockchain = Blockchain::new();
    let utxo_set = UtxoSet::new();

    assert!(blockchain.is_valid());
    assert!(utxo_set.is_empty());
}

#[test]
fn transaction_basic_validation_is_enforced() {
    let tx = Transaction::new(
        1,
        Vec::new(),
        vec![TransactionOutput {
            amount: 100,
            recipient: vec![2u8; 33],
        }],
        0,
    );

    assert!(!tx.validate_basic());
}

#[test]
fn empty_transaction_block_is_accepted() {
    let blockchain = Blockchain::new();
    let block = blockchain.genesis();

    assert!(block.transactions.is_empty());
    assert!(block.has_valid_merkle_root());
}

#[test]
fn transaction_output_cannot_be_zero() {
    let mut utxo_set = UtxoSet::new();

    let original_utxo = Utxo {
        id: UtxoId {
            transaction_id: [1u8; 32],
            output_index: 0,
        },
        amount: 1000,
        recipient: vec![1u8; 33],
    };

    utxo_set
        .insert(original_utxo)
        .expect("UTXO insertion should succeed");

    let input = TransactionInput {
        previous_output: [1u8; 32],
        output_index: 0,
        public_key: Vec::new(),
        signature: Vec::new(),
    };

    let output = TransactionOutput {
        amount: 0,
        recipient: vec![2u8; 33],
    };

    let tx = Transaction::new(1, vec![input], vec![output], 1000);

    assert!(utxo_set.validate_transaction(&tx).is_err());
}

#[test]
fn duplicate_inputs_are_detected() {
    let mut utxo_set = UtxoSet::new();

    let original_utxo = Utxo {
        id: UtxoId {
            transaction_id: [1u8; 32],
            output_index: 0,
        },
        amount: 1000,
        recipient: vec![1u8; 33],
    };

    utxo_set
        .insert(original_utxo)
        .expect("UTXO insertion should succeed");

    let input_1 = TransactionInput {
        previous_output: [1u8; 32],
        output_index: 0,
        public_key: Vec::new(),
        signature: Vec::new(),
    };

    let input_2 = TransactionInput {
        previous_output: [1u8; 32],
        output_index: 0,
        public_key: Vec::new(),
        signature: Vec::new(),
    };

    let output = TransactionOutput {
        amount: 900,
        recipient: vec![2u8; 33],
    };

    let tx = Transaction::new(1, vec![input_1, input_2], vec![output], 100);

    assert!(utxo_set.validate_transaction(&tx).is_err());
}
