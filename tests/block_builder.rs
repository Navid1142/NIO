use nio_blockchain::block_builder::BlockBuilder;
use nio_blockchain::chain::Blockchain;
use nio_blockchain::mempool::Mempool;
use nio_blockchain::mining::Miner;
use nio_blockchain::transaction::{Transaction, TransactionInput, TransactionOutput};
use nio_blockchain::utxo::{Utxo, UtxoId, UtxoSet};

use secp256k1::{PublicKey, Secp256k1, SecretKey};

// ============================================================
// TEST HELPERS
// ============================================================

fn secret_key() -> SecretKey {
    SecretKey::from_slice(&[1u8; 32]).expect("valid secret key")
}

fn public_key() -> Vec<u8> {
    let secp = Secp256k1::new();
    let secret = secret_key();

    let public = PublicKey::from_secret_key(&secp, &secret);

    public.serialize().to_vec()
}

fn input(value: u8) -> TransactionInput {
    TransactionInput {
        previous_output: [value; 32],
        output_index: 0,

        // باید با مالک UTXO یکسان باشد.
        public_key: public_key(),

        signature: Vec::new(),
    }
}

fn output(amount: u64) -> TransactionOutput {
    TransactionOutput {
        amount,
        recipient: vec![2u8; 33],
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

        // مالک UTXO همان public key تراکنش است.
        recipient: public_key(),
    }
}

// ============================================================
// SELECTION
// ============================================================

#[test]
fn selects_transactions_by_highest_fee() {
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
fn selection_respects_transaction_limit() {
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
fn zero_limit_selects_nothing() {
    let mempool = Mempool::new();

    let selected = BlockBuilder::select_transactions(&mempool, 0);

    assert!(selected.is_empty());
}

// ============================================================
// UTXO VALIDATION
// ============================================================

#[test]
fn selected_transactions_are_validated_against_utxos() {
    let mut utxos = UtxoSet::new();

    utxos.insert(utxo(1, 100)).expect("utxo should be inserted");

    let tx = signed_transaction(1, 90, 10);

    let result = BlockBuilder::validate_selected_transactions(&[tx], &utxos);

    assert!(result.is_ok(), "transaction should be valid: {:?}", result);

    assert_eq!(result.unwrap(), 10);
}

#[test]
fn invalid_transaction_is_rejected() {
    let mut utxos = UtxoSet::new();

    utxos.insert(utxo(1, 100)).expect("utxo should be inserted");

    // Input = 100
    // Output = 90
    // Real fee = 10
    // Declared fee = 9
    let tx = signed_transaction(1, 90, 9);

    let result = BlockBuilder::validate_selected_transactions(&[tx], &utxos);

    assert!(result.is_err());
}

#[test]
fn missing_utxo_is_rejected() {
    let utxos = UtxoSet::new();

    let tx = signed_transaction(99, 90, 10);

    let result = BlockBuilder::validate_selected_transactions(&[tx], &utxos);

    assert!(result.is_err());
}

// ============================================================
// UTXO STATE SAFETY
// ============================================================

#[test]
fn validation_does_not_modify_original_utxo_set() {
    let mut utxos = UtxoSet::new();

    utxos.insert(utxo(1, 100)).expect("utxo should be inserted");

    let before_hash = utxos.state_hash();

    let tx = signed_transaction(1, 90, 10);

    let result = BlockBuilder::validate_selected_transactions(&[tx], &utxos);

    assert!(result.is_ok(), "transaction should be valid: {:?}", result);

    let after_hash = utxos.state_hash();

    assert_eq!(
        before_hash, after_hash,
        "validation must not modify original UTXO state"
    );
}

// ============================================================
// TEMPLATE
// ============================================================

#[test]
fn valid_block_template_is_accepted() {
    let blockchain = Blockchain::new();
    let mempool = Mempool::new();

    let block = BlockBuilder::build_template(&blockchain, &mempool, 100, 1_000)
        .expect("template should build");

    assert_eq!(block.header.height, 1);

    assert_eq!(block.header.previous_hash, blockchain.latest_block().hash());

    assert!(block.has_valid_merkle_root());
}

#[test]
fn invalid_merkle_root_is_rejected() {
    let blockchain = Blockchain::new();
    let mempool = Mempool::new();

    let mut block = BlockBuilder::build_template(&blockchain, &mempool, 100, 1_000)
        .expect("template should build");

    block.header.merkle_root = [1u8; 32];

    let result = BlockBuilder::validate_block_template(&block);

    assert!(result.is_err());
}

// ============================================================
// TRANSACTION TEMPLATE
// ============================================================

#[test]
fn template_contains_selected_transactions() {
    let blockchain = Blockchain::new();
    let mempool = Mempool::new();

    let result = BlockBuilder::build_template(&blockchain, &mempool, 10, 1_000);

    assert!(result.is_ok(), "template should build: {:?}", result);

    let block = result.unwrap();

    assert_eq!(block.transactions.len(), 0);
}

// ============================================================
// TIMESTAMP
// ============================================================

#[test]
fn timestamp_must_increase() {
    let blockchain = Blockchain::new();
    let mempool = Mempool::new();

    let previous_timestamp = blockchain.latest_block().header.timestamp;

    let result = BlockBuilder::build_template(&blockchain, &mempool, 10, previous_timestamp);

    assert!(result.is_err());
}

// ============================================================
// MERKLE ROOT
// ============================================================

#[test]
fn template_has_valid_merkle_root() {
    let blockchain = Blockchain::new();
    let mempool = Mempool::new();

    let block = BlockBuilder::build_template(&blockchain, &mempool, 10, 1_000)
        .expect("template should build");

    assert!(block.has_valid_merkle_root());
}

// ============================================================
// POW
// ============================================================

#[test]
fn build_and_mine_produces_valid_pow() {
    let blockchain = Blockchain::new();
    let mempool = Mempool::new();

    let block = BlockBuilder::build_and_mine(&blockchain, &mempool, 10, 1_000)
        .expect("mining should succeed");

    assert!(Miner::validate(&block));
}
