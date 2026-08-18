use crate::transaction::{Transaction, TransactionId};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UtxoId {
    pub transaction_id: TransactionId,
    pub output_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Utxo {
    pub id: UtxoId,
    pub amount: u64,
    pub recipient: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UtxoSet {
    entries: HashMap<UtxoId, Utxo>,
}

impl UtxoSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Utxo> {
        self.entries.values()
    }

    pub fn get(&self, id: &UtxoId) -> Option<&Utxo> {
        self.entries.get(id)
    }

    pub fn contains(&self, id: &UtxoId) -> bool {
        self.entries.contains_key(id)
    }

    // ============================================================
    // CANONICAL STATE HASH
    // ============================================================

    pub fn state_hash(&self) -> [u8; 32] {
        let mut entries: Vec<&Utxo> = self.entries.values().collect();

        entries.sort_by(|a, b| {
            a.id.transaction_id
                .cmp(&b.id.transaction_id)
                .then_with(|| a.id.output_index.cmp(&b.id.output_index))
        });

        let mut hasher = Sha256::new();

        hasher.update(b"NIO-UTXO-STATE-V1");
        hasher.update((entries.len() as u64).to_le_bytes());

        for utxo in entries {
            hasher.update(utxo.id.transaction_id);
            hasher.update(utxo.id.output_index.to_le_bytes());
            hasher.update(utxo.amount.to_le_bytes());
            hasher.update((utxo.recipient.len() as u64).to_le_bytes());
            hasher.update(&utxo.recipient);
        }

        let result = hasher.finalize();

        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);

        hash
    }

    pub fn state_equals(&self, other: &UtxoSet) -> bool {
        self == other
    }

    pub fn has_state_hash(&self, expected: [u8; 32]) -> bool {
        self.state_hash() == expected
    }

    // ============================================================
    // INSERT
    // ============================================================

    pub fn insert(&mut self, utxo: Utxo) -> Result<(), String> {
        if utxo.amount == 0 {
            return Err("UTXO amount cannot be zero".to_string());
        }

        if utxo.recipient.len() != 33 {
            return Err("UTXO recipient must be exactly 33 bytes".to_string());
        }

        if self.entries.contains_key(&utxo.id) {
            return Err("UTXO already exists".to_string());
        }

        self.entries.insert(utxo.id.clone(), utxo);

        Ok(())
    }

    // ============================================================
    // SPEND
    // ============================================================

    pub fn spend(&mut self, id: &UtxoId) -> Result<Utxo, String> {
        self.entries
            .remove(id)
            .ok_or_else(|| "UTXO does not exist or was already spent".to_string())
    }

    // ============================================================
    // INPUT VALUE
    // ============================================================

    pub fn input_value(&self, transaction: &Transaction) -> Result<u64, String> {
        let mut total = 0u64;
        let mut seen = HashSet::<UtxoId>::new();

        for input in &transaction.inputs {
            let id = UtxoId {
                transaction_id: input.previous_output,
                output_index: input.output_index,
            };

            if !seen.insert(id.clone()) {
                return Err("duplicate input detected".to_string());
            }

            let utxo = self
                .get(&id)
                .ok_or_else(|| "input UTXO does not exist".to_string())?;

            total = total
                .checked_add(utxo.amount)
                .ok_or_else(|| "input value overflow".to_string())?;
        }

        Ok(total)
    }

    // ============================================================
    // OUTPUT VALUE
    // ============================================================

    pub fn output_value(transaction: &Transaction) -> Result<u64, String> {
        let mut total = 0u64;

        for output in &transaction.outputs {
            if output.amount == 0 {
                return Err("output amount cannot be zero".to_string());
            }

            if output.recipient.is_empty() {
                return Err("output recipient cannot be empty".to_string());
            }

            total = total
                .checked_add(output.amount)
                .ok_or_else(|| "output value overflow".to_string())?;
        }

        Ok(total)
    }

    // ============================================================
    // ACTUAL TRANSACTION FEE
    // ============================================================

    pub fn calculate_fee(&self, transaction: &Transaction) -> Result<u64, String> {
        if transaction.is_coinbase() {
            return Err("coinbase has no transaction fee".to_string());
        }

        let input_total = self.input_value(transaction)?;
        let output_total = Self::output_value(transaction)?;

        if input_total < output_total {
            return Err("outputs exceed inputs".to_string());
        }

        Ok(input_total - output_total)
    }

    // ============================================================
    // FEE VALIDATION
    // ============================================================

    pub fn validate_fee(&self, transaction: &Transaction) -> Result<u64, String> {
        if transaction.is_coinbase() {
            return Err("coinbase cannot use normal transaction fee validation".to_string());
        }

        let actual_fee = self.calculate_fee(transaction)?;

        if actual_fee != transaction.fee {
            return Err(format!(
                "invalid transaction fee: declared {}, actual {}",
                transaction.fee, actual_fee
            ));
        }

        Ok(actual_fee)
    }

    // ============================================================
    // TRANSACTION VALIDATION
    // ============================================================

    pub fn validate_transaction(&self, transaction: &Transaction) -> Result<(), String> {
        if transaction.is_coinbase() {
            return Err("coinbase must be validated separately".to_string());
        }

        if !transaction.validate_basic() {
            return Err("basic transaction validation failed".to_string());
        }

        if transaction.inputs.is_empty() {
            return Err("transaction must contain inputs".to_string());
        }

        if transaction.outputs.is_empty() {
            return Err("transaction must contain outputs".to_string());
        }

        let mut seen_inputs = HashSet::<UtxoId>::new();

        for input in &transaction.inputs {
            let id = UtxoId {
                transaction_id: input.previous_output,
                output_index: input.output_index,
            };

            if !seen_inputs.insert(id.clone()) {
                return Err("duplicate input detected".to_string());
            }

            let utxo = self
                .get(&id)
                .ok_or_else(|| "input UTXO does not exist".to_string())?;

            // ====================================================
            // OWNERSHIP VALIDATION
            // ====================================================
            //
            // The public key used to sign the transaction MUST
            // match the public key recorded as the UTXO recipient.
            //
            // This prevents a valid signature from an unrelated
            // private key from spending someone else's UTXO.
            //

            if utxo.recipient != input.public_key {
                return Err("input public key does not own UTXO".to_string());
            }
        }

        // Cryptographic signature validation.
        if !transaction.validate_signatures() {
            return Err("invalid transaction signatures".to_string());
        }

        // Exact fee validation.
        self.validate_fee(transaction)?;

        Ok(())
    }

    // ============================================================
    // ATOMIC TRANSACTION APPLY
    // ============================================================

    pub fn apply_transaction(&mut self, transaction: &Transaction) -> Result<u64, String> {
        // Validate everything before changing state.
        self.validate_transaction(transaction)?;

        let fee = self.validate_fee(transaction)?;
        let transaction_id = transaction.id();

        let input_ids: Vec<UtxoId> = transaction
            .inputs
            .iter()
            .map(|input| UtxoId {
                transaction_id: input.previous_output,
                output_index: input.output_index,
            })
            .collect();

        let mut new_utxos = Vec::<Utxo>::new();

        for (index, output) in transaction.outputs.iter().enumerate() {
            if output.amount == 0 {
                return Err("output amount cannot be zero".to_string());
            }

            if output.recipient.is_empty() {
                return Err("output recipient cannot be empty".to_string());
            }

            let output_index =
                u32::try_from(index).map_err(|_| "too many transaction outputs".to_string())?;

            let id = UtxoId {
                transaction_id,
                output_index,
            };

            if self.contains(&id) {
                return Err("transaction output UTXO already exists".to_string());
            }

            new_utxos.push(Utxo {
                id,
                amount: output.amount,
                recipient: output.recipient.clone(),
            });
        }

        // --------------------------------------------------------
        // STATE TRANSITION
        // --------------------------------------------------------

        for id in &input_ids {
            self.spend(id)?;
        }

        for utxo in new_utxos {
            self.insert(utxo)?;
        }

        Ok(fee)
    }
}

// ================================================================
// TESTS
// ================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use secp256k1::{PublicKey, Secp256k1, SecretKey};

    // ============================================================
    // TEST HELPERS
    // ============================================================

    fn tx_id(value: u8) -> TransactionId {
        [value; 32]
    }

    fn secret_key() -> SecretKey {
        SecretKey::from_slice(&[1u8; 32]).expect("valid test secret key")
    }

    fn second_secret_key() -> SecretKey {
        SecretKey::from_slice(&[2u8; 32]).expect("valid second test secret key")
    }

    fn public_key(secret: &SecretKey) -> Vec<u8> {
        let secp = Secp256k1::new();

        PublicKey::from_secret_key(&secp, secret)
            .serialize()
            .to_vec()
    }

    fn make_utxo(value: u8, amount: u64) -> Utxo {
        Utxo {
            id: UtxoId {
                transaction_id: tx_id(value),
                output_index: 0,
            },
            amount,
            recipient: public_key(&secret_key()),
        }
    }

    fn make_owned_utxo(value: u8, amount: u64, secret: &SecretKey) -> Utxo {
        Utxo {
            id: UtxoId {
                transaction_id: tx_id(value),
                output_index: 0,
            },
            amount,
            recipient: public_key(secret),
        }
    }

    fn make_input(value: u8) -> crate::transaction::TransactionInput {
        crate::transaction::TransactionInput {
            previous_output: tx_id(value),
            output_index: 0,
            public_key: Vec::new(),
            signature: Vec::new(),
        }
    }

    fn make_output(amount: u64) -> crate::transaction::TransactionOutput {
        crate::transaction::TransactionOutput {
            amount,
            recipient: vec![2u8; 33],
        }
    }

    fn signed_transaction(input_value: u8, output_amount: u64, fee: u64) -> Transaction {
        let mut tx = Transaction::new(
            1,
            vec![make_input(input_value)],
            vec![make_output(output_amount)],
            fee,
        );

        tx.sign_input(0, &secret_key())
            .expect("transaction must sign");

        tx
    }

    // ============================================================
    // BASIC UTXO TESTS
    // ============================================================

    #[test]
    fn utxo_set_starts_empty() {
        let set = UtxoSet::new();

        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
    }

    #[test]
    fn utxo_can_be_inserted() {
        let mut set = UtxoSet::new();

        set.insert(make_utxo(1, 1000))
            .expect("insert should succeed");

        assert_eq!(set.len(), 1);
        assert!(!set.is_empty());
    }

    #[test]
    fn iterator_returns_utxos() {
        let mut set = UtxoSet::new();

        set.insert(make_utxo(1, 100)).expect("insert");

        set.insert(make_utxo(2, 200)).expect("insert");

        let total: u64 = set.iter().map(|utxo| utxo.amount).sum();

        assert_eq!(total, 300);
    }

    #[test]
    fn zero_utxo_is_rejected() {
        let mut set = UtxoSet::new();

        assert!(set.insert(make_utxo(1, 0)).is_err());
    }

    #[test]
    fn duplicate_utxo_is_rejected() {
        let mut set = UtxoSet::new();

        set.insert(make_utxo(1, 100)).expect("first insert");

        assert!(set.insert(make_utxo(1, 100)).is_err());
    }

    // ============================================================
    // STATE INTEGRITY TESTS
    // ============================================================

    #[test]
    fn identical_utxo_sets_are_equal() {
        let mut first = UtxoSet::new();
        let mut second = UtxoSet::new();

        first.insert(make_utxo(1, 100)).expect("insert");

        second.insert(make_utxo(1, 100)).expect("insert");

        assert_eq!(first, second);
        assert!(first.state_equals(&second));
    }

    #[test]
    fn different_utxo_amount_is_detected() {
        let mut first = UtxoSet::new();
        let mut second = UtxoSet::new();

        first.insert(make_utxo(1, 100)).expect("insert");

        second.insert(make_utxo(1, 200)).expect("insert");

        assert_ne!(first, second);
        assert!(!first.state_equals(&second));
    }

    #[test]
    fn different_utxo_id_is_detected() {
        let mut first = UtxoSet::new();
        let mut second = UtxoSet::new();

        first.insert(make_utxo(1, 100)).expect("insert");

        second.insert(make_utxo(2, 100)).expect("insert");

        assert_ne!(first, second);
    }

    #[test]
    fn different_recipient_is_detected() {
        let mut first = UtxoSet::new();
        let mut second = UtxoSet::new();

        first.insert(make_utxo(1, 100)).expect("insert");

        second
            .insert(Utxo {
                id: UtxoId {
                    transaction_id: tx_id(1),
                    output_index: 0,
                },
                amount: 100,
                recipient: vec![9u8; 33],
            })
            .expect("insert");

        assert_ne!(first, second);
    }

    #[test]
    fn state_hash_is_deterministic() {
        let mut first = UtxoSet::new();
        let mut second = UtxoSet::new();

        first.insert(make_utxo(1, 100)).expect("insert");

        first.insert(make_utxo(2, 200)).expect("insert");

        second.insert(make_utxo(2, 200)).expect("insert");

        second.insert(make_utxo(1, 100)).expect("insert");

        assert_eq!(first.state_hash(), second.state_hash());
    }

    #[test]
    fn state_hash_changes_when_state_changes() {
        let mut set = UtxoSet::new();

        set.insert(make_utxo(1, 100)).expect("insert");

        let original_hash = set.state_hash();

        set.insert(make_utxo(2, 200)).expect("insert");

        let changed_hash = set.state_hash();

        assert_ne!(original_hash, changed_hash);
    }

    #[test]
    fn empty_state_has_stable_hash() {
        let first = UtxoSet::new();
        let second = UtxoSet::new();

        assert_eq!(first.state_hash(), second.state_hash());
    }

    // ============================================================
    // FEE TESTS
    // ============================================================

    #[test]
    fn exact_fee_is_calculated() {
        let mut set = UtxoSet::new();

        set.insert(make_utxo(1, 1000)).expect("insert");

        let tx = signed_transaction(1, 900, 100);

        assert_eq!(set.calculate_fee(&tx).expect("fee calculation"), 100);
    }

    #[test]
    fn correct_fee_is_accepted() {
        let mut set = UtxoSet::new();

        set.insert(make_utxo(1, 1000)).expect("insert");

        let tx = signed_transaction(1, 900, 100);

        assert!(set.validate_fee(&tx).is_ok());
    }

    #[test]
    fn incorrect_fee_is_rejected() {
        let mut set = UtxoSet::new();

        set.insert(make_utxo(1, 1000)).expect("insert");

        let tx = signed_transaction(1, 900, 99);

        assert!(set.validate_fee(&tx).is_err());
    }

    #[test]
    fn hidden_value_cannot_disappear() {
        let mut set = UtxoSet::new();

        set.insert(make_utxo(1, 1000)).expect("insert");

        let tx = signed_transaction(1, 900, 50);

        assert!(set.validate_transaction(&tx).is_err());
    }

    // ============================================================
    // TRANSACTION APPLY
    // ============================================================

    #[test]
    fn exact_fee_transaction_is_applied() {
        let mut set = UtxoSet::new();

        set.insert(make_utxo(1, 1000)).expect("insert");

        let tx = signed_transaction(1, 900, 100);

        let fee = set
            .apply_transaction(&tx)
            .expect("transaction should apply");

        assert_eq!(fee, 100);
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn double_spend_is_rejected() {
        let mut set = UtxoSet::new();

        set.insert(make_utxo(1, 1000)).expect("insert");

        let tx = signed_transaction(1, 900, 100);

        assert!(set.apply_transaction(&tx).is_ok());

        let second = signed_transaction(1, 900, 100);

        assert!(set.apply_transaction(&second).is_err());
    }

    #[test]
    fn insufficient_input_is_rejected() {
        let mut set = UtxoSet::new();

        set.insert(make_utxo(1, 100)).expect("insert");

        let tx = signed_transaction(1, 101, 0);

        assert!(set.validate_transaction(&tx).is_err());
    }

    #[test]
    fn failed_transaction_is_atomic() {
        let mut set = UtxoSet::new();

        let original = make_utxo(1, 1000);
        let original_id = original.id.clone();

        set.insert(original).expect("insert");

        let tx = signed_transaction(1, 900, 50);
        let original_hash = set.state_hash();

        assert!(set.apply_transaction(&tx).is_err());

        assert!(set.contains(&original_id));
        assert_eq!(set.len(), 1);

        assert_eq!(set.state_hash(), original_hash);
    }

    // ============================================================
    // OWNERSHIP SECURITY TESTS
    // ============================================================

    #[test]
    fn correct_owner_can_spend_utxo() {
        let owner = secret_key();

        let mut set = UtxoSet::new();

        set.insert(make_owned_utxo(1, 1000, &owner))
            .expect("UTXO insert should succeed");

        let mut tx = Transaction::new(1, vec![make_input(1)], vec![make_output(900)], 100);

        tx.sign_input(0, &owner)
            .expect("owner should be able to sign");

        assert!(set.validate_transaction(&tx).is_ok());
    }

    #[test]
    fn wrong_public_key_cannot_spend_utxo() {
        let owner = secret_key();
        let attacker = second_secret_key();

        let mut set = UtxoSet::new();

        set.insert(make_owned_utxo(1, 1000, &owner))
            .expect("UTXO insert should succeed");

        let mut tx = Transaction::new(1, vec![make_input(1)], vec![make_output(900)], 100);

        tx.sign_input(0, &attacker)
            .expect("attacker can create an ECDSA signature");

        assert!(set.validate_transaction(&tx).is_err());

        // State must remain unchanged.
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn changing_public_key_to_non_owner_is_rejected() {
        let owner = secret_key();
        let attacker = second_secret_key();

        let mut set = UtxoSet::new();

        set.insert(make_owned_utxo(1, 1000, &owner))
            .expect("UTXO insert");

        let mut tx = Transaction::new(1, vec![make_input(1)], vec![make_output(900)], 100);

        tx.sign_input(0, &owner).expect("owner signature");

        assert!(set.validate_transaction(&tx).is_ok());

        // Replace public key with attacker key.
        tx.inputs[0].public_key = public_key(&attacker);

        assert!(set.validate_transaction(&tx).is_err());
    }

    // ============================================================
    // ATTACK / FAILURE TESTS
    // ============================================================

    #[test]
    fn tampered_utxo_amount_changes_state_hash() {
        let mut set = UtxoSet::new();

        set.insert(make_utxo(1, 1000))
            .expect("UTXO insert should succeed");

        let original_hash = set.state_hash();

        let id = UtxoId {
            transaction_id: tx_id(1),
            output_index: 0,
        };

        let utxo = set.entries.get_mut(&id).expect("UTXO must exist");

        utxo.amount = 999;

        assert_ne!(set.state_hash(), original_hash);
    }

    #[test]
    fn tampered_utxo_recipient_changes_state_hash() {
        let mut set = UtxoSet::new();

        set.insert(make_utxo(1, 1000))
            .expect("UTXO insert should succeed");

        let original_hash = set.state_hash();

        let id = UtxoId {
            transaction_id: tx_id(1),
            output_index: 0,
        };

        let utxo = set.entries.get_mut(&id).expect("UTXO must exist");

        utxo.recipient = vec![9u8; 33];

        assert_ne!(set.state_hash(), original_hash);
    }

    #[test]
    fn tampered_utxo_id_changes_state_hash() {
        let mut set = UtxoSet::new();

        set.insert(make_utxo(1, 1000))
            .expect("UTXO insert should succeed");

        let original_hash = set.state_hash();

        let original = set
            .entries
            .remove(&UtxoId {
                transaction_id: tx_id(1),
                output_index: 0,
            })
            .expect("UTXO must exist");

        let tampered = Utxo {
            id: UtxoId {
                transaction_id: tx_id(99),
                output_index: 0,
            },
            amount: original.amount,
            recipient: original.recipient,
        };

        set.entries.insert(tampered.id.clone(), tampered);

        assert_ne!(set.state_hash(), original_hash);
    }

    #[test]
    fn tampered_state_is_not_equal_to_original() {
        let mut original = UtxoSet::new();

        original.insert(make_utxo(1, 1000)).expect("insert");

        let mut tampered = original.clone();

        let id = UtxoId {
            transaction_id: tx_id(1),
            output_index: 0,
        };

        tampered
            .entries
            .get_mut(&id)
            .expect("UTXO must exist")
            .amount = 999;

        assert!(!original.state_equals(&tampered));
        assert_ne!(original.state_hash(), tampered.state_hash());
    }
}
