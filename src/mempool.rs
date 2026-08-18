use crate::transaction::{Transaction, TransactionId};
use crate::utxo::{UtxoId, UtxoSet};
use std::collections::{HashMap, HashSet};

const DEFAULT_MAX_TRANSACTIONS: usize = 5_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MempoolConfig {
    pub max_transactions: usize,
    pub min_fee: u64,
}

impl Default for MempoolConfig {
    fn default() -> Self {
        Self {
            max_transactions: DEFAULT_MAX_TRANSACTIONS,
            min_fee: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MempoolError {
    TransactionAlreadyExists,
    MempoolFull,
    FeeTooLow,
    InvalidTransaction,
    InvalidSignature,
    MissingUtxo,
    InsufficientFunds,
    DuplicateInput,
    DoubleSpend,
}

#[derive(Debug, Clone)]
pub struct Mempool {
    transactions: HashMap<TransactionId, Transaction>,
    reserved_inputs: HashMap<UtxoId, TransactionId>,
    config: MempoolConfig,
}

impl Default for Mempool {
    fn default() -> Self {
        Self::new()
    }
}

impl Mempool {
    pub fn new() -> Self {
        Self::with_config(MempoolConfig::default())
    }

    pub fn with_config(config: MempoolConfig) -> Self {
        Self {
            transactions: HashMap::new(),
            reserved_inputs: HashMap::new(),
            config,
        }
    }

    pub fn len(&self) -> usize {
        self.transactions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.config.max_transactions
    }

    pub fn min_fee(&self) -> u64 {
        self.config.min_fee
    }

    pub fn contains(&self, id: &TransactionId) -> bool {
        self.transactions.contains_key(id)
    }

    pub fn get(&self, id: &TransactionId) -> Option<&Transaction> {
        self.transactions.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Transaction> {
        self.transactions.values()
    }

    // ============================================================
    // BASIC ADD
    // ============================================================

    pub fn add(&mut self, transaction: Transaction) -> Result<TransactionId, MempoolError> {
        if !transaction.validate_basic() {
            return Err(MempoolError::InvalidTransaction);
        }

        if transaction.fee < self.config.min_fee {
            return Err(MempoolError::FeeTooLow);
        }

        let id = transaction.id();

        if self.transactions.contains_key(&id) {
            return Err(MempoolError::TransactionAlreadyExists);
        }

        if self.transactions.len() >= self.config.max_transactions {
            return Err(MempoolError::MempoolFull);
        }

        self.transactions.insert(id, transaction);

        Ok(id)
    }

    // ============================================================
    // FULL UTXO ADMISSION
    // ============================================================

    pub fn add_with_utxo(
        &mut self,
        transaction: Transaction,
        utxos: &UtxoSet,
    ) -> Result<TransactionId, MempoolError> {
        if !transaction.validate_basic() {
            return Err(MempoolError::InvalidTransaction);
        }

        if transaction.fee < self.config.min_fee {
            return Err(MempoolError::FeeTooLow);
        }

        let id = transaction.id();

        if self.transactions.contains_key(&id) {
            return Err(MempoolError::TransactionAlreadyExists);
        }

        if self.transactions.len() >= self.config.max_transactions {
            return Err(MempoolError::MempoolFull);
        }

        // Signature validation.
        if !transaction.validate_signatures() {
            return Err(MempoolError::InvalidSignature);
        }

        let mut input_ids = Vec::with_capacity(transaction.inputs.len());
        let mut seen = HashSet::new();

        for input in &transaction.inputs {
            let utxo_id = UtxoId {
                transaction_id: input.previous_output,
                output_index: input.output_index,
            };

            // Duplicate input inside the same transaction.
            if !seen.insert(utxo_id.clone()) {
                return Err(MempoolError::DuplicateInput);
            }

            // UTXO must exist.
            if !utxos.contains(&utxo_id) {
                return Err(MempoolError::MissingUtxo);
            }

            // UTXO cannot already be reserved.
            if self.reserved_inputs.contains_key(&utxo_id) {
                return Err(MempoolError::DoubleSpend);
            }

            input_ids.push(utxo_id);
        }

        // Full UTXO validation.
        match utxos.validate_transaction(&transaction) {
            Ok(()) => {}
            Err(_) => match utxos.input_value(&transaction) {
                Ok(input_total) => match UtxoSet::output_value(&transaction) {
                    Ok(output_total) => match output_total.checked_add(transaction.fee) {
                        Some(required) if input_total >= required => {
                            return Err(MempoolError::InvalidTransaction);
                        }
                        _ => {
                            return Err(MempoolError::InsufficientFunds);
                        }
                    },
                    Err(_) => {
                        return Err(MempoolError::InvalidTransaction);
                    }
                },
                Err(_) => {
                    return Err(MempoolError::MissingUtxo);
                }
            },
        }

        // Reserve all inputs only after every validation succeeds.
        for utxo_id in input_ids {
            self.reserved_inputs.insert(utxo_id, id);
        }

        self.transactions.insert(id, transaction);

        Ok(id)
    }

    // ============================================================
    // REMOVE
    // ============================================================

    pub fn remove(&mut self, id: &TransactionId) -> Option<Transaction> {
        let transaction = self.transactions.remove(id)?;

        self.release_inputs(&transaction, id);

        Some(transaction)
    }

    fn release_inputs(&mut self, transaction: &Transaction, transaction_id: &TransactionId) {
        for input in &transaction.inputs {
            let utxo_id = UtxoId {
                transaction_id: input.previous_output,
                output_index: input.output_index,
            };

            if self.reserved_inputs.get(&utxo_id) == Some(transaction_id) {
                self.reserved_inputs.remove(&utxo_id);
            }
        }
    }

    // ============================================================
    // RESERVATION
    // ============================================================

    pub fn is_input_reserved(&self, id: &UtxoId) -> bool {
        self.reserved_inputs.contains_key(id)
    }

    pub fn reserved_by(&self, id: &UtxoId) -> Option<&TransactionId> {
        self.reserved_inputs.get(id)
    }

    pub fn reserved_input_count(&self) -> usize {
        self.reserved_inputs.len()
    }

    // ============================================================
    // CLEAR
    // ============================================================

    pub fn clear(&mut self) {
        self.transactions.clear();
        self.reserved_inputs.clear();
    }

    // ============================================================
    // REMOVE MANY
    // ============================================================

    pub fn remove_many(&mut self, ids: &[TransactionId]) -> usize {
        let mut removed = 0;

        for id in ids {
            if self.remove(id).is_some() {
                removed += 1;
            }
        }

        removed
    }

    // ============================================================
    // FEE ORDERING
    // ============================================================

    pub fn transactions_by_fee_desc(&self) -> Vec<&Transaction> {
        let mut transactions: Vec<&Transaction> = self.transactions.values().collect();

        transactions.sort_by(|a, b| b.fee.cmp(&a.fee).then_with(|| b.id().cmp(&a.id())));

        transactions
    }

    // ============================================================
    // TAKE BEST
    // ============================================================

    pub fn take_best(&mut self, limit: usize) -> Vec<Transaction> {
        if limit == 0 {
            return Vec::new();
        }

        let ids: Vec<TransactionId> = self
            .transactions_by_fee_desc()
            .into_iter()
            .take(limit)
            .map(|tx| tx.id())
            .collect();

        let mut selected = Vec::with_capacity(ids.len());

        for id in ids {
            if let Some(tx) = self.remove(&id) {
                selected.push(tx);
            }
        }

        selected
    }
}

// ================================================================
// TESTS
// ================================================================

#[cfg(test)]
mod tests {
    use super::*;

    use crate::transaction::{TransactionInput, TransactionOutput};

    use secp256k1::{PublicKey, Secp256k1, SecretKey};

    fn secret_key() -> SecretKey {
        SecretKey::from_slice(&[1u8; 32]).expect("valid secret key")
    }

    fn public_key() -> Vec<u8> {
        let secp = Secp256k1::new();

        PublicKey::from_secret_key(&secp, &secret_key())
            .serialize()
            .to_vec()
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
            recipient: vec![2u8; 33],
        }
    }

    fn transaction(value: u8, fee: u64) -> Transaction {
        Transaction::new(1, vec![input(value)], vec![output(100)], fee)
    }

    fn signed_transaction(value: u8, output_amount: u64, fee: u64) -> Transaction {
        let mut tx = Transaction::new(1, vec![input(value)], vec![output(output_amount)], fee);

        tx.sign_input(0, &secret_key())
            .expect("transaction should be signed");

        tx
    }

    fn utxo(value: u8, amount: u64) -> crate::utxo::Utxo {
        crate::utxo::Utxo {
            id: UtxoId {
                transaction_id: [value; 32],
                output_index: 0,
            },
            amount,
            recipient: public_key(),
        }
    }

    // ============================================================
    // BASIC
    // ============================================================

    #[test]
    fn mempool_starts_empty() {
        let mempool = Mempool::new();

        assert!(mempool.is_empty());
        assert_eq!(mempool.len(), 0);
        assert_eq!(mempool.reserved_input_count(), 0);
    }

    #[test]
    fn valid_transaction_can_be_added() {
        let mut mempool = Mempool::new();

        let tx = transaction(1, 10);
        let id = tx.id();

        assert_eq!(mempool.add(tx).expect("transaction should be accepted"), id);

        assert_eq!(mempool.len(), 1);
        assert!(mempool.contains(&id));
    }

    #[test]
    fn duplicate_transaction_is_rejected() {
        let mut mempool = Mempool::new();

        let tx = transaction(1, 10);

        assert!(mempool.add(tx.clone()).is_ok());

        assert_eq!(mempool.add(tx), Err(MempoolError::TransactionAlreadyExists));
    }

    #[test]
    fn low_fee_transaction_is_rejected() {
        let config = MempoolConfig {
            max_transactions: 100,
            min_fee: 10,
        };

        let mut mempool = Mempool::with_config(config);

        assert_eq!(mempool.add(transaction(1, 9)), Err(MempoolError::FeeTooLow));

        assert!(mempool.is_empty());
    }

    #[test]
    fn minimum_fee_is_accepted() {
        let config = MempoolConfig {
            max_transactions: 100,
            min_fee: 10,
        };

        let mut mempool = Mempool::with_config(config);

        assert!(mempool.add(transaction(1, 10)).is_ok());
    }

    #[test]
    fn capacity_limit_is_enforced() {
        let config = MempoolConfig {
            max_transactions: 2,
            min_fee: 0,
        };

        let mut mempool = Mempool::with_config(config);

        assert!(mempool.add(transaction(1, 1)).is_ok());
        assert!(mempool.add(transaction(2, 2)).is_ok());

        assert_eq!(
            mempool.add(transaction(3, 3)),
            Err(MempoolError::MempoolFull)
        );

        assert_eq!(mempool.len(), 2);
    }

    // ============================================================
    // UTXO ADMISSION
    // ============================================================

    #[test]
    fn signed_transaction_with_valid_utxo_is_accepted() {
        let mut mempool = Mempool::new();
        let mut utxos = UtxoSet::new();

        utxos
            .insert(utxo(1, 1000))
            .expect("utxo insertion should succeed");

        let tx = signed_transaction(1, 900, 100);
        let id = tx.id();

        assert_eq!(
            mempool
                .add_with_utxo(tx, &utxos)
                .expect("transaction should be accepted"),
            id
        );

        assert_eq!(mempool.len(), 1);
        assert_eq!(mempool.reserved_input_count(), 1);

        let reserved_id = UtxoId {
            transaction_id: [1u8; 32],
            output_index: 0,
        };

        assert!(mempool.is_input_reserved(&reserved_id));

        assert!(utxos.contains(&reserved_id));
    }

    #[test]
    fn unsigned_transaction_is_rejected_by_real_admission() {
        let mut mempool = Mempool::new();
        let mut utxos = UtxoSet::new();

        utxos
            .insert(utxo(1, 1000))
            .expect("utxo insertion should succeed");

        let tx = transaction(1, 100);

        assert_eq!(
            mempool.add_with_utxo(tx, &utxos),
            Err(MempoolError::InvalidSignature)
        );

        assert!(mempool.is_empty());
        assert_eq!(mempool.reserved_input_count(), 0);
    }

    #[test]
    fn missing_utxo_is_rejected() {
        let mut mempool = Mempool::new();
        let utxos = UtxoSet::new();

        let tx = signed_transaction(99, 900, 100);

        assert_eq!(
            mempool.add_with_utxo(tx, &utxos),
            Err(MempoolError::MissingUtxo)
        );
    }

    #[test]
    fn insufficient_funds_are_rejected() {
        let mut mempool = Mempool::new();
        let mut utxos = UtxoSet::new();

        utxos
            .insert(utxo(1, 100))
            .expect("utxo insertion should succeed");

        let tx = signed_transaction(1, 100, 1);

        assert_eq!(
            mempool.add_with_utxo(tx, &utxos),
            Err(MempoolError::InsufficientFunds)
        );

        assert!(mempool.is_empty());
        assert_eq!(mempool.reserved_input_count(), 0);
    }

    #[test]
    fn mempool_double_spend_is_rejected() {
        let mut mempool = Mempool::new();
        let mut utxos = UtxoSet::new();

        utxos
            .insert(utxo(1, 1000))
            .expect("utxo insertion should succeed");

        let tx1 = signed_transaction(1, 900, 100);

        assert!(mempool.add_with_utxo(tx1, &utxos).is_ok());

        let tx2 = signed_transaction(1, 800, 200);

        assert_eq!(
            mempool.add_with_utxo(tx2, &utxos),
            Err(MempoolError::DoubleSpend)
        );

        assert_eq!(mempool.len(), 1);
        assert_eq!(mempool.reserved_input_count(), 1);
    }

    #[test]
    fn removing_transaction_releases_reserved_utxo() {
        let mut mempool = Mempool::new();
        let mut utxos = UtxoSet::new();

        utxos
            .insert(utxo(1, 1000))
            .expect("utxo insertion should succeed");

        let tx = signed_transaction(1, 900, 100);
        let id = tx.id();

        mempool
            .add_with_utxo(tx, &utxos)
            .expect("transaction should be accepted");

        assert_eq!(mempool.reserved_input_count(), 1);

        mempool.remove(&id);

        assert_eq!(mempool.reserved_input_count(), 0);

        let reserved_id = UtxoId {
            transaction_id: [1u8; 32],
            output_index: 0,
        };

        assert!(!mempool.is_input_reserved(&reserved_id));
    }

    // ============================================================
    // ORDERING
    // ============================================================

    #[test]
    fn best_transactions_are_ordered_by_fee() {
        let mut mempool = Mempool::new();

        mempool
            .add(transaction(1, 5))
            .expect("transaction should be accepted");

        mempool
            .add(transaction(2, 50))
            .expect("transaction should be accepted");

        mempool
            .add(transaction(3, 20))
            .expect("transaction should be accepted");

        let ordered = mempool.transactions_by_fee_desc();

        assert_eq!(ordered[0].fee, 50);
        assert_eq!(ordered[1].fee, 20);
        assert_eq!(ordered[2].fee, 5);
    }

    #[test]
    fn take_best_removes_highest_fee_transactions() {
        let mut mempool = Mempool::new();

        mempool
            .add(transaction(1, 5))
            .expect("transaction should be accepted");

        mempool
            .add(transaction(2, 50))
            .expect("transaction should be accepted");

        mempool
            .add(transaction(3, 20))
            .expect("transaction should be accepted");

        let selected = mempool.take_best(2);

        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].fee, 50);
        assert_eq!(selected[1].fee, 20);
        assert_eq!(mempool.len(), 1);
    }

    #[test]
    fn take_best_zero_returns_nothing() {
        let mut mempool = Mempool::new();

        mempool
            .add(transaction(1, 10))
            .expect("transaction should be accepted");

        let selected = mempool.take_best(0);

        assert!(selected.is_empty());
        assert_eq!(mempool.len(), 1);
    }

    #[test]
    fn clear_removes_all_transactions_and_reservations() {
        let mut mempool = Mempool::new();
        let mut utxos = UtxoSet::new();

        utxos
            .insert(utxo(1, 1000))
            .expect("utxo insertion should succeed");

        let tx = signed_transaction(1, 900, 100);

        mempool
            .add_with_utxo(tx, &utxos)
            .expect("transaction should be accepted");

        assert_eq!(mempool.reserved_input_count(), 1);

        mempool.clear();

        assert!(mempool.is_empty());
        assert_eq!(mempool.reserved_input_count(), 0);
    }

    #[test]
    fn remove_many_only_removes_existing_transactions() {
        let mut mempool = Mempool::new();

        let tx1 = transaction(1, 10);
        let tx2 = transaction(2, 20);

        let id1 = tx1.id();
        let id2 = tx2.id();

        mempool.add(tx1).expect("transaction should be accepted");

        mempool.add(tx2).expect("transaction should be accepted");

        let removed = mempool.remove_many(&[id1, id2, [99; 32]]);

        assert_eq!(removed, 2);
        assert!(mempool.is_empty());
    }
}
