use nio_blockchain::{Transaction, TransactionInput, TransactionOutput, Utxo, UtxoId, UtxoSet};
use secp256k1::{PublicKey, Secp256k1, SecretKey};

fn owner_key() -> SecretKey {
    SecretKey::from_slice(&[1u8; 32]).expect("owner key must be valid")
}

fn attacker_key() -> SecretKey {
    SecretKey::from_slice(&[2u8; 32]).expect("attacker key must be valid")
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

fn make_input(value: u8) -> TransactionInput {
    TransactionInput {
        previous_output: tx_id(value),
        output_index: 0,
        public_key: Vec::new(),
        signature: Vec::new(),
    }
}

fn make_output(amount: u64) -> TransactionOutput {
    TransactionOutput {
        amount,
        recipient: vec![9u8; 33],
    }
}

fn make_owned_utxo(value: u8, amount: u64, owner: &SecretKey) -> Utxo {
    Utxo {
        id: UtxoId {
            transaction_id: tx_id(value),
            output_index: 0,
        },
        amount,
        recipient: public_key(owner),
    }
}

// ============================================================
// 1. WRONG OWNER ATTACK
// ============================================================

#[test]
fn attacker_cannot_spend_someone_elses_utxo() {
    let owner = owner_key();
    let attacker = attacker_key();

    let mut set = UtxoSet::new();

    set.insert(make_owned_utxo(1, 1000, &owner))
        .expect("UTXO insertion should succeed");

    let mut tx = Transaction::new(1, vec![make_input(1)], vec![make_output(900)], 100);

    tx.sign_input(0, &attacker)
        .expect("attacker can create a valid signature");

    assert!(
        set.validate_transaction(&tx).is_err(),
        "attacker must not be able to spend owner's UTXO"
    );

    assert_eq!(set.len(), 1);
}

// ============================================================
// 2. OUTPUT TAMPERING
// ============================================================

#[test]
fn changing_output_after_signing_is_rejected() {
    let owner = owner_key();

    let mut set = UtxoSet::new();

    set.insert(make_owned_utxo(2, 1000, &owner))
        .expect("UTXO insertion should succeed");

    let mut tx = Transaction::new(1, vec![make_input(2)], vec![make_output(900)], 100);

    tx.sign_input(0, &owner)
        .expect("transaction should be signed");

    assert!(
        set.validate_transaction(&tx).is_ok(),
        "original transaction must be valid"
    );

    // Attack: change the output after signing.
    tx.outputs[0].amount = 800;

    assert!(
        set.validate_transaction(&tx).is_err(),
        "tampered output must be rejected"
    );

    // State must remain unchanged.
    assert_eq!(set.len(), 1);
}

// ============================================================
// 3. FEE TAMPERING
// ============================================================

#[test]
fn changing_fee_after_signing_is_rejected() {
    let owner = owner_key();

    let mut set = UtxoSet::new();

    set.insert(make_owned_utxo(3, 1000, &owner))
        .expect("UTXO insertion should succeed");

    let mut tx = Transaction::new(1, vec![make_input(3)], vec![make_output(900)], 100);

    tx.sign_input(0, &owner)
        .expect("transaction should be signed");

    assert!(
        set.validate_transaction(&tx).is_ok(),
        "original transaction must be valid"
    );

    // Attack: modify declared fee.
    tx.fee = 99;

    assert!(
        set.validate_transaction(&tx).is_err(),
        "tampered fee must be rejected"
    );

    assert_eq!(set.len(), 1);
}

// ============================================================
// 4. PREVIOUS OUTPUT TAMPERING
// ============================================================

#[test]
fn changing_previous_output_after_signing_is_rejected() {
    let owner = owner_key();

    let mut set = UtxoSet::new();

    set.insert(make_owned_utxo(4, 1000, &owner))
        .expect("UTXO insertion should succeed");

    let mut tx = Transaction::new(1, vec![make_input(4)], vec![make_output(900)], 100);

    tx.sign_input(0, &owner)
        .expect("transaction should be signed");

    assert!(
        set.validate_transaction(&tx).is_ok(),
        "original transaction must be valid"
    );

    // Attack: redirect the input to another UTXO.
    tx.inputs[0].previous_output = tx_id(99);

    assert!(
        set.validate_transaction(&tx).is_err(),
        "tampered previous output must be rejected"
    );

    // Original UTXO must still exist.
    let original_id = UtxoId {
        transaction_id: tx_id(4),
        output_index: 0,
    };

    assert!(set.contains(&original_id));
}

// ============================================================
// 5. SIGNATURE CORRUPTION
// ============================================================

#[test]
fn corrupted_signature_is_rejected() {
    let owner = owner_key();

    let mut set = UtxoSet::new();

    set.insert(make_owned_utxo(5, 1000, &owner))
        .expect("UTXO insertion should succeed");

    let mut tx = Transaction::new(1, vec![make_input(5)], vec![make_output(900)], 100);

    tx.sign_input(0, &owner)
        .expect("transaction should be signed");

    assert!(
        set.validate_transaction(&tx).is_ok(),
        "original transaction must be valid"
    );

    // Attack: corrupt one byte of the signature.
    tx.inputs[0].signature[0] ^= 0x01;

    assert!(
        set.validate_transaction(&tx).is_err(),
        "corrupted signature must be rejected"
    );

    assert_eq!(set.len(), 1);
}
// ============================================================
// LEVEL 2 — ADVANCED ATTACK / FAILURE TESTS
// ============================================================

// ------------------------------------------------------------
// 6. DOUBLE-SPEND ATTACK
// ------------------------------------------------------------

#[test]
fn double_spend_attack_is_rejected() {
    let owner = owner_key();

    let mut set = UtxoSet::new();

    set.insert(make_owned_utxo(6, 1000, &owner))
        .expect("UTXO insertion should succeed");

    let mut first_tx = Transaction::new(1, vec![make_input(6)], vec![make_output(900)], 100);

    first_tx
        .sign_input(0, &owner)
        .expect("first transaction should be signed");

    assert!(
        set.apply_transaction(&first_tx).is_ok(),
        "first spend should succeed"
    );

    // Attempt to spend the exact same original UTXO again.
    let mut second_tx = Transaction::new(
        1,
        vec![make_input(6)],
        vec![TransactionOutput {
            amount: 800,
            recipient: vec![8u8; 33],
        }],
        200,
    );

    second_tx
        .sign_input(0, &owner)
        .expect("second transaction should be signable");

    assert!(
        set.apply_transaction(&second_tx).is_err(),
        "double-spend must be rejected"
    );
}

// ------------------------------------------------------------
// 7. DUPLICATE INPUT ATTACK
// ------------------------------------------------------------

#[test]
fn duplicate_input_attack_is_rejected() {
    let owner = owner_key();

    let mut set = UtxoSet::new();

    set.insert(make_owned_utxo(7, 1000, &owner))
        .expect("UTXO insertion should succeed");

    let mut tx = Transaction::new(
        1,
        vec![make_input(7), make_input(7)],
        vec![TransactionOutput {
            amount: 900,
            recipient: vec![7u8; 33],
        }],
        100,
    );

    tx.sign_input(0, &owner)
        .expect("first input should be signed");

    tx.sign_input(1, &owner)
        .expect("second input should be signed");

    assert!(
        set.validate_transaction(&tx).is_err(),
        "duplicate inputs must be rejected"
    );

    // State must remain untouched.
    assert_eq!(set.len(), 1);
}

// ------------------------------------------------------------
// 8. CREATE MONEY FROM NOTHING
// ------------------------------------------------------------

#[test]
fn outputs_exceeding_inputs_are_rejected() {
    let owner = owner_key();

    let mut set = UtxoSet::new();

    set.insert(make_owned_utxo(8, 1000, &owner))
        .expect("UTXO insertion should succeed");

    // Input = 1000
    // Output = 1100
    // Declared fee = 0
    //
    // This would create 100 NIO from nothing.
    let mut tx = Transaction::new(
        1,
        vec![make_input(8)],
        vec![TransactionOutput {
            amount: 1100,
            recipient: vec![6u8; 33],
        }],
        0,
    );

    tx.sign_input(0, &owner)
        .expect("transaction should be signed");

    assert!(
        set.validate_transaction(&tx).is_err(),
        "transaction creating value from nothing must be rejected"
    );

    assert_eq!(set.len(), 1);
}

// ------------------------------------------------------------
// 9. INTEGER OVERFLOW ATTACK
// ------------------------------------------------------------

#[test]
fn output_value_overflow_is_rejected() {
    let transaction = Transaction::new(
        1,
        vec![make_input(9)],
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
        UtxoSet::output_value(&transaction).is_err(),
        "output value overflow must be rejected"
    );
}

// ------------------------------------------------------------
// 10. INPUT VALUE OVERFLOW
// ------------------------------------------------------------

#[test]
fn input_value_overflow_is_rejected() {
    let owner = owner_key();

    let mut set = UtxoSet::new();

    set.insert(make_owned_utxo(10, u64::MAX, &owner))
        .expect("first UTXO should be inserted");

    set.insert(Utxo {
        id: UtxoId {
            transaction_id: tx_id(11),
            output_index: 0,
        },
        amount: 1,
        recipient: public_key(&owner),
    })
    .expect("second UTXO should be inserted");

    let tx = Transaction::new(
        1,
        vec![make_input(10), make_input(11)],
        vec![TransactionOutput {
            amount: 1,
            recipient: vec![3u8; 33],
        }],
        0,
    );

    assert!(
        set.input_value(&tx).is_err(),
        "input value overflow must be rejected"
    );

    // State must remain unchanged.
    assert_eq!(set.len(), 2);
}

// ------------------------------------------------------------
// 11. ATOMIC FAILURE
// ------------------------------------------------------------

#[test]
fn failed_transaction_cannot_modify_state() {
    let owner = owner_key();

    let mut set = UtxoSet::new();

    set.insert(make_owned_utxo(12, 1000, &owner))
        .expect("UTXO insertion should succeed");

    let original_hash = set.state_hash();
    let original_len = set.len();

    // Invalid transaction: output exceeds input.
    let mut tx = Transaction::new(
        1,
        vec![make_input(12)],
        vec![TransactionOutput {
            amount: 2000,
            recipient: vec![4u8; 33],
        }],
        0,
    );

    tx.sign_input(0, &owner)
        .expect("transaction should be signed");

    assert!(
        set.apply_transaction(&tx).is_err(),
        "invalid transaction must fail"
    );

    // The failed transaction must not alter state.
    assert_eq!(set.len(), original_len);
    assert_eq!(set.state_hash(), original_hash);

    let original_id = UtxoId {
        transaction_id: tx_id(12),
        output_index: 0,
    };

    assert!(
        set.contains(&original_id),
        "original UTXO must remain after failed transaction"
    );
}

// ------------------------------------------------------------
// 12. INVALID OUTPUT INDEX
// ------------------------------------------------------------

#[test]
fn invalid_output_index_cannot_spend_utxo() {
    let owner = owner_key();

    let mut set = UtxoSet::new();

    set.insert(make_owned_utxo(13, 1000, &owner))
        .expect("UTXO insertion should succeed");

    let mut tx = Transaction::new(
        1,
        vec![TransactionInput {
            previous_output: tx_id(13),
            output_index: 999,
            public_key: Vec::new(),
            signature: Vec::new(),
        }],
        vec![make_output(900)],
        100,
    );

    tx.sign_input(0, &owner)
        .expect("transaction should be signed");

    assert!(
        set.validate_transaction(&tx).is_err(),
        "invalid output index must be rejected"
    );

    assert_eq!(set.len(), 1);
}
