use nio_blockchain::{Transaction, TransactionInput, TransactionOutput, Utxo, UtxoId, UtxoSet};
use secp256k1::{PublicKey, Secp256k1, SecretKey};

fn owner_key() -> SecretKey {
    SecretKey::from_slice(&[1u8; 32]).expect("valid owner key")
}

fn public_key(secret: &SecretKey) -> Vec<u8> {
    let secp = Secp256k1::new();

    PublicKey::from_secret_key(&secp, secret)
        .serialize()
        .to_vec()
}

fn tx_id(value: u8) -> [u8; 32] {
    [value; 32]
}

fn input(value: u8) -> TransactionInput {
    TransactionInput {
        previous_output: tx_id(value),
        output_index: 0,
        public_key: Vec::new(),
        signature: Vec::new(),
    }
}

// ============================================================
// 1. ZERO-VALUE OUTPUT
// ============================================================

#[test]
fn zero_value_output_is_rejected() {
    let tx = Transaction::new(
        1,
        vec![input(1)],
        vec![TransactionOutput {
            amount: 0,
            recipient: vec![2u8; 33],
        }],
        0,
    );

    assert!(!tx.validate_basic(), "zero-value output must be rejected");

    assert!(
        UtxoSet::output_value(&tx).is_err(),
        "zero-value output must not be accepted by UTXO validation"
    );
}

// ============================================================
// 2. EMPTY RECIPIENT
// ============================================================

#[test]
fn empty_recipient_is_rejected() {
    let tx = Transaction::new(
        1,
        vec![input(2)],
        vec![TransactionOutput {
            amount: 100,
            recipient: Vec::new(),
        }],
        0,
    );

    assert!(!tx.validate_basic(), "empty recipient must be rejected");

    assert!(
        UtxoSet::output_value(&tx).is_err(),
        "empty recipient must be rejected by UTXO validation"
    );
}

// ============================================================
// 3. INVALID RECIPIENT LENGTH
// ============================================================

#[test]
fn invalid_recipient_length_is_detected() {
    let mut set = UtxoSet::new();

    let invalid_recipient = vec![7u8; 32];

    let result = set.insert(Utxo {
        id: UtxoId {
            transaction_id: tx_id(3),
            output_index: 0,
        },
        amount: 100,
        recipient: invalid_recipient,
    });

    /*
     * Current UTXO implementation only rejects empty recipients.
     *
     * This test documents the security requirement that an NIO
     * public-key recipient must be exactly 33 bytes.
     */
    assert!(
        result.is_err(),
        "UTXO recipient with invalid public-key length must be rejected"
    );
}

// ============================================================
// 4. INVALID PUBLIC KEY IN TRANSACTION INPUT
// ============================================================

#[test]
fn invalid_public_key_cannot_pass_signature_validation() {
    let owner = owner_key();

    let mut tx = Transaction::new(
        1,
        vec![input(4)],
        vec![TransactionOutput {
            amount: 900,
            recipient: vec![2u8; 33],
        }],
        100,
    );

    tx.sign_input(0, &owner)
        .expect("transaction should be signed");

    // Corrupt public key.
    tx.inputs[0].public_key = vec![9u8; 33];

    assert!(
        !tx.validate_signatures(),
        "invalid public key must invalidate transaction"
    );
}

// ============================================================
// 5. INVALID SIGNATURE LENGTH
// ============================================================

#[test]
fn invalid_signature_length_is_rejected() {
    let owner = owner_key();

    let mut tx = Transaction::new(
        1,
        vec![input(5)],
        vec![TransactionOutput {
            amount: 900,
            recipient: vec![2u8; 33],
        }],
        100,
    );

    tx.sign_input(0, &owner)
        .expect("transaction should be signed");

    tx.inputs[0].signature = vec![1u8; 10];

    assert!(
        !tx.validate_signatures(),
        "invalid signature length must be rejected"
    );
}

// ============================================================
// 6. COINBASE ZERO REWARD
// ============================================================

#[test]
fn zero_coinbase_reward_is_rejected() {
    assert!(
        Transaction::coinbase(0, vec![1u8; 33]).is_err(),
        "zero coinbase reward must be rejected"
    );
}

// ============================================================
// 7. COINBASE EMPTY RECIPIENT
// ============================================================

#[test]
fn coinbase_empty_recipient_is_rejected() {
    assert!(
        Transaction::coinbase(100, Vec::new()).is_err(),
        "coinbase with empty recipient must be rejected"
    );
}

// ============================================================
// 8. COINBASE FEE
// ============================================================

#[test]
fn coinbase_fee_must_be_zero() {
    let tx = Transaction {
        version: 1,
        inputs: Vec::new(),
        outputs: vec![TransactionOutput {
            amount: 100,
            recipient: vec![1u8; 33],
        }],
        fee: 1,
    };

    assert!(
        !tx.validate_coinbase(),
        "coinbase must never contain a normal transaction fee"
    );
}

// ============================================================
// 9. MULTIPLE COINBASE OUTPUTS
// ============================================================

#[test]
fn multiple_coinbase_outputs_are_rejected() {
    let tx = Transaction {
        version: 1,
        inputs: Vec::new(),
        outputs: vec![
            TransactionOutput {
                amount: 100,
                recipient: vec![1u8; 33],
            },
            TransactionOutput {
                amount: 50,
                recipient: vec![2u8; 33],
            },
        ],
        fee: 0,
    };

    assert!(
        !tx.validate_coinbase(),
        "coinbase must contain exactly one output"
    );
}

// ============================================================
// 10. STATE HASH CHANGES AFTER TAMPERING
// ============================================================

#[test]
fn state_hash_detects_utxo_tampering() {
    let owner = owner_key();

    let mut set = UtxoSet::new();

    set.insert(Utxo {
        id: UtxoId {
            transaction_id: tx_id(10),
            output_index: 0,
        },
        amount: 1000,
        recipient: public_key(&owner),
    })
    .expect("UTXO insertion should succeed");

    let original_hash = set.state_hash();

    let id = UtxoId {
        transaction_id: tx_id(10),
        output_index: 0,
    };

    /*
     * We cannot directly mutate private UtxoSet state from an
     * integration test, so verify that a different canonical
     * state produces a different hash.
     */

    let mut modified = UtxoSet::new();

    modified
        .insert(Utxo {
            id,
            amount: 999,
            recipient: public_key(&owner),
        })
        .expect("modified UTXO should be inserted");

    assert_ne!(
        original_hash,
        modified.state_hash(),
        "state hash must detect changed UTXO amount"
    );
}

// ============================================================
// 11. DUPLICATE INPUT SECURITY
// ============================================================

#[test]
fn duplicate_inputs_are_rejected_before_state_change() {
    let owner = owner_key();

    let mut set = UtxoSet::new();

    set.insert(Utxo {
        id: UtxoId {
            transaction_id: tx_id(11),
            output_index: 0,
        },
        amount: 1000,
        recipient: public_key(&owner),
    })
    .expect("UTXO insertion should succeed");

    let mut tx = Transaction::new(
        1,
        vec![input(11), input(11)],
        vec![TransactionOutput {
            amount: 900,
            recipient: vec![3u8; 33],
        }],
        100,
    );

    tx.sign_input(0, &owner).expect("first input should sign");

    tx.sign_input(1, &owner).expect("second input should sign");

    let before_hash = set.state_hash();

    assert!(
        set.validate_transaction(&tx).is_err(),
        "duplicate inputs must be rejected"
    );

    assert_eq!(
        set.state_hash(),
        before_hash,
        "rejected transaction must not modify state"
    );
}

// ============================================================
// 12. OUTPUT OVERFLOW
// ============================================================

#[test]
fn output_overflow_is_rejected() {
    let tx = Transaction::new(
        1,
        vec![input(12)],
        vec![
            TransactionOutput {
                amount: u64::MAX,
                recipient: vec![1u8; 33],
            },
            TransactionOutput {
                amount: 1,
                recipient: vec![2u8; 33],
            },
        ],
        0,
    );

    assert!(
        UtxoSet::output_value(&tx).is_err(),
        "output overflow must be rejected"
    );
}

// ============================================================
// 13. INPUT OVERFLOW
// ============================================================

#[test]
fn input_overflow_is_rejected() {
    let owner = owner_key();

    let mut set = UtxoSet::new();

    set.insert(Utxo {
        id: UtxoId {
            transaction_id: tx_id(13),
            output_index: 0,
        },
        amount: u64::MAX,
        recipient: public_key(&owner),
    })
    .expect("first UTXO should be inserted");

    set.insert(Utxo {
        id: UtxoId {
            transaction_id: tx_id(14),
            output_index: 0,
        },
        amount: 1,
        recipient: public_key(&owner),
    })
    .expect("second UTXO should be inserted");

    let tx = Transaction::new(
        1,
        vec![input(13), input(14)],
        vec![TransactionOutput {
            amount: 1,
            recipient: vec![4u8; 33],
        }],
        0,
    );

    assert!(
        set.input_value(&tx).is_err(),
        "input overflow must be rejected"
    );
}
