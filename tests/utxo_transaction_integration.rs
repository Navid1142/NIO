use nio_blockchain::{Transaction, TransactionInput, TransactionOutput, Utxo, UtxoId, UtxoSet};
use secp256k1::{PublicKey, Secp256k1, SecretKey};

fn secret_key() -> SecretKey {
    SecretKey::from_slice(&[1u8; 32]).expect("valid secret key")
}

fn public_key(secret: &SecretKey) -> Vec<u8> {
    let secp = Secp256k1::new();

    PublicKey::from_secret_key(&secp, secret)
        .serialize()
        .to_vec()
}

fn transaction_id(value: u8) -> [u8; 32] {
    [value; 32]
}

#[test]
fn complete_transaction_flow_works() {
    let owner = secret_key();
    let owner_public_key = public_key(&owner);

    // ------------------------------------------------------------
    // 1. Create initial UTXO
    // ------------------------------------------------------------

    let mut utxo_set = UtxoSet::new();

    let initial_utxo = Utxo {
        id: UtxoId {
            transaction_id: transaction_id(1),
            output_index: 0,
        },
        amount: 1000,
        recipient: owner_public_key.clone(),
    };

    utxo_set
        .insert(initial_utxo)
        .expect("initial UTXO should be inserted");

    assert_eq!(utxo_set.len(), 1);

    // ------------------------------------------------------------
    // 2. Create transaction
    // ------------------------------------------------------------

    let mut transaction = Transaction::new(
        1,
        vec![TransactionInput {
            previous_output: transaction_id(1),
            output_index: 0,
            public_key: Vec::new(),
            signature: Vec::new(),
        }],
        vec![TransactionOutput {
            amount: 900,
            recipient: vec![2u8; 33],
        }],
        100,
    );

    // ------------------------------------------------------------
    // 3. Sign transaction
    // ------------------------------------------------------------

    transaction
        .sign_input(0, &owner)
        .expect("transaction should be signed");

    assert_eq!(transaction.inputs[0].public_key, owner_public_key);
    assert_eq!(transaction.inputs[0].signature.len(), 64);

    // ------------------------------------------------------------
    // 4. Verify signature
    // ------------------------------------------------------------

    assert!(transaction
        .verify_input(0)
        .expect("signature verification should work"));

    // ------------------------------------------------------------
    // 5. Validate complete transaction
    // ------------------------------------------------------------

    assert!(utxo_set.validate_transaction(&transaction).is_ok());

    // ------------------------------------------------------------
    // 6. Apply transaction
    // ------------------------------------------------------------

    let fee = utxo_set
        .apply_transaction(&transaction)
        .expect("transaction should apply");

    assert_eq!(fee, 100);

    // ------------------------------------------------------------
    // 7. Original UTXO must be spent
    // ------------------------------------------------------------

    let original_id = UtxoId {
        transaction_id: transaction_id(1),
        output_index: 0,
    };

    assert!(!utxo_set.contains(&original_id));

    // ------------------------------------------------------------
    // 8. New UTXO must exist
    // ------------------------------------------------------------

    let new_id = UtxoId {
        transaction_id: transaction.id(),
        output_index: 0,
    };

    assert!(utxo_set.contains(&new_id));

    let new_utxo = utxo_set.get(&new_id).expect("new UTXO should exist");

    assert_eq!(new_utxo.amount, 900);
    assert_eq!(new_utxo.recipient, vec![2u8; 33]);
}

#[test]
fn double_spend_is_rejected() {
    let owner = secret_key();

    let mut utxo_set = UtxoSet::new();

    utxo_set
        .insert(Utxo {
            id: UtxoId {
                transaction_id: transaction_id(10),
                output_index: 0,
            },
            amount: 1000,
            recipient: public_key(&owner),
        })
        .expect("UTXO should be inserted");

    let mut transaction = Transaction::new(
        1,
        vec![TransactionInput {
            previous_output: transaction_id(10),
            output_index: 0,
            public_key: Vec::new(),
            signature: Vec::new(),
        }],
        vec![TransactionOutput {
            amount: 900,
            recipient: vec![3u8; 33],
        }],
        100,
    );

    transaction
        .sign_input(0, &owner)
        .expect("transaction should be signed");

    // First spend succeeds.
    assert!(utxo_set.apply_transaction(&transaction).is_ok());

    // Second spend of the same original UTXO must fail.
    let mut second_transaction = Transaction::new(
        1,
        vec![TransactionInput {
            previous_output: transaction_id(10),
            output_index: 0,
            public_key: Vec::new(),
            signature: Vec::new(),
        }],
        vec![TransactionOutput {
            amount: 900,
            recipient: vec![4u8; 33],
        }],
        100,
    );

    second_transaction
        .sign_input(0, &owner)
        .expect("second transaction can be signed");

    assert!(utxo_set.apply_transaction(&second_transaction).is_err());
}

#[test]
fn wrong_owner_cannot_spend_utxo() {
    let owner = secret_key();

    let attacker = SecretKey::from_slice(&[2u8; 32]).expect("valid attacker key");

    let mut utxo_set = UtxoSet::new();

    utxo_set
        .insert(Utxo {
            id: UtxoId {
                transaction_id: transaction_id(20),
                output_index: 0,
            },
            amount: 1000,
            recipient: public_key(&owner),
        })
        .expect("UTXO should be inserted");

    let mut transaction = Transaction::new(
        1,
        vec![TransactionInput {
            previous_output: transaction_id(20),
            output_index: 0,
            public_key: Vec::new(),
            signature: Vec::new(),
        }],
        vec![TransactionOutput {
            amount: 900,
            recipient: vec![5u8; 33],
        }],
        100,
    );

    transaction
        .sign_input(0, &attacker)
        .expect("attacker can create a signature");

    assert!(utxo_set.validate_transaction(&transaction).is_err());

    // UTXO must remain untouched.
    assert_eq!(utxo_set.len(), 1);
}
