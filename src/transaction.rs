use secp256k1::{ecdsa::Signature, Message, PublicKey, Secp256k1, SecretKey};
use sha2::{Digest, Sha256};

pub type TransactionId = [u8; 32];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionInput {
    pub previous_output: TransactionId,
    pub output_index: u32,
    pub public_key: Vec<u8>,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionOutput {
    pub amount: u64,
    pub recipient: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub version: u32,
    pub inputs: Vec<TransactionInput>,
    pub outputs: Vec<TransactionOutput>,
    pub fee: u64,
}

impl Transaction {
    pub fn new(
        version: u32,
        inputs: Vec<TransactionInput>,
        outputs: Vec<TransactionOutput>,
        fee: u64,
    ) -> Self {
        Self {
            version,
            inputs,
            outputs,
            fee,
        }
    }

    // ============================================================
    // BASIC VALIDATION
    // ============================================================

    pub fn validate_basic(&self) -> bool {
        // Normal transactions must have inputs.
        if self.inputs.is_empty() {
            return false;
        }

        // Normal transactions must have outputs.
        if self.outputs.is_empty() {
            return false;
        }

        // Every output must have:
        // - a non-zero amount
        // - exactly 33 bytes compressed public-key recipient
        if self
            .outputs
            .iter()
            .any(|output| output.amount == 0 || output.recipient.len() != 33)
        {
            return false;
        }

        // Reject duplicate inputs.
        for i in 0..self.inputs.len() {
            for j in (i + 1)..self.inputs.len() {
                if self.inputs[i].previous_output == self.inputs[j].previous_output
                    && self.inputs[i].output_index == self.inputs[j].output_index
                {
                    return false;
                }
            }
        }

        true
    }

    // ============================================================
    // COINBASE
    // ============================================================

    pub fn coinbase(reward: u64, recipient: Vec<u8>) -> Result<Self, String> {
        if reward == 0 {
            return Err("coinbase reward cannot be zero".to_string());
        }

        if recipient.is_empty() {
            return Err("coinbase recipient cannot be empty".to_string());
        }

        Ok(Self {
            version: 1,
            inputs: Vec::new(),
            outputs: vec![TransactionOutput {
                amount: reward,
                recipient,
            }],
            fee: 0,
        })
    }

    pub fn is_coinbase(&self) -> bool {
        self.inputs.is_empty()
    }

    pub fn validate_coinbase(&self) -> bool {
        if !self.is_coinbase() {
            return false;
        }

        if self.outputs.len() != 1 {
            return false;
        }

        if self.outputs[0].amount == 0 {
            return false;
        }

        if self.outputs[0].recipient.is_empty() {
            return false;
        }

        if self.fee != 0 {
            return false;
        }

        true
    }

    // ============================================================
    // TRANSACTION ID
    // ============================================================

    pub fn id(&self) -> TransactionId {
        let mut hasher = Sha256::new();

        hasher.update(self.version.to_le_bytes());

        hasher.update((self.inputs.len() as u64).to_le_bytes());

        for input in &self.inputs {
            hasher.update(input.previous_output);
            hasher.update(input.output_index.to_le_bytes());

            hasher.update((input.public_key.len() as u64).to_le_bytes());
            hasher.update(&input.public_key);

            hasher.update((input.signature.len() as u64).to_le_bytes());
            hasher.update(&input.signature);
        }

        hasher.update((self.outputs.len() as u64).to_le_bytes());

        for output in &self.outputs {
            hasher.update(output.amount.to_le_bytes());

            hasher.update((output.recipient.len() as u64).to_le_bytes());
            hasher.update(&output.recipient);
        }

        hasher.update(self.fee.to_le_bytes());

        let digest = hasher.finalize();

        let mut id = [0u8; 32];
        id.copy_from_slice(&digest);

        id
    }

    // ============================================================
    // SIGNING HASH
    // ============================================================

    fn signing_hash(&self, input_index: usize, public_key: &[u8]) -> Result<TransactionId, String> {
        if input_index >= self.inputs.len() {
            return Err("invalid input index".to_string());
        }

        let mut hasher = Sha256::new();

        hasher.update(self.version.to_le_bytes());

        hasher.update((self.inputs.len() as u64).to_le_bytes());

        for (index, input) in self.inputs.iter().enumerate() {
            hasher.update(input.previous_output);
            hasher.update(input.output_index.to_le_bytes());

            if index == input_index {
                hasher.update((public_key.len() as u64).to_le_bytes());
                hasher.update(public_key);
            }
        }

        hasher.update((self.outputs.len() as u64).to_le_bytes());

        for output in &self.outputs {
            hasher.update(output.amount.to_le_bytes());

            hasher.update((output.recipient.len() as u64).to_le_bytes());
            hasher.update(&output.recipient);
        }

        hasher.update(self.fee.to_le_bytes());

        let digest = hasher.finalize();

        let mut hash = [0u8; 32];
        hash.copy_from_slice(&digest);

        Ok(hash)
    }

    // ============================================================
    // SIGN INPUT
    // ============================================================

    pub fn sign_input(&mut self, input_index: usize, secret_key: &SecretKey) -> Result<(), String> {
        if self.is_coinbase() {
            return Err("coinbase transaction cannot be signed".to_string());
        }

        if input_index >= self.inputs.len() {
            return Err("invalid input index".to_string());
        }

        let secp = Secp256k1::new();

        let public_key = PublicKey::from_secret_key(&secp, secret_key);

        let public_key_bytes = public_key.serialize();

        let hash = self.signing_hash(input_index, &public_key_bytes)?;

        let message = Message::from_digest(hash);

        let signature = secp.sign_ecdsa(&message, secret_key);

        self.inputs[input_index].public_key = public_key_bytes.to_vec();

        self.inputs[input_index].signature = signature.serialize_compact().to_vec();

        Ok(())
    }

    // ============================================================
    // VERIFY INPUT
    // ============================================================

    pub fn verify_input(&self, input_index: usize) -> Result<bool, String> {
        if input_index >= self.inputs.len() {
            return Err("invalid input index".to_string());
        }

        let input = &self.inputs[input_index];

        if input.public_key.len() != 33 {
            return Ok(false);
        }

        if input.signature.len() != 64 {
            return Ok(false);
        }

        let public_key = match PublicKey::from_slice(&input.public_key) {
            Ok(key) => key,
            Err(_) => return Ok(false),
        };

        let signature = match Signature::from_compact(&input.signature) {
            Ok(sig) => sig,
            Err(_) => return Ok(false),
        };

        let hash = self.signing_hash(input_index, &input.public_key)?;

        let message = Message::from_digest(hash);

        let secp = Secp256k1::verification_only();

        Ok(secp.verify_ecdsa(&message, &signature, &public_key).is_ok())
    }

    // ============================================================
    // VERIFY ALL SIGNATURES
    // ============================================================

    pub fn verify_signatures(&self) -> Result<(), String> {
        if self.is_coinbase() {
            return Err("coinbase transaction has no signatures".to_string());
        }

        for index in 0..self.inputs.len() {
            if !self.verify_input(index)? {
                return Err(format!("invalid signature at input {}", index));
            }
        }

        Ok(())
    }

    // ============================================================
    // COMPLETE SIGNATURE VALIDATION
    // ============================================================

    pub fn validate_signatures(&self) -> bool {
        if self.is_coinbase() {
            return false;
        }

        if !self.validate_basic() {
            return false;
        }

        self.verify_signatures().is_ok()
    }
}

// ================================================================
// TESTS
// ================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn secret_key() -> SecretKey {
        SecretKey::from_slice(&[1u8; 32]).expect("valid test secret key")
    }

    fn second_secret_key() -> SecretKey {
        SecretKey::from_slice(&[2u8; 32]).expect("valid second test secret key")
    }

    fn input() -> TransactionInput {
        TransactionInput {
            previous_output: [7u8; 32],
            output_index: 0,
            public_key: Vec::new(),
            signature: Vec::new(),
        }
    }

    fn second_input() -> TransactionInput {
        TransactionInput {
            previous_output: [8u8; 32],
            output_index: 0,
            public_key: Vec::new(),
            signature: Vec::new(),
        }
    }

    fn output() -> TransactionOutput {
        TransactionOutput {
            amount: 900,
            recipient: vec![2u8; 33],
        }
    }

    fn transaction() -> Transaction {
        Transaction::new(1, vec![input()], vec![output()], 100)
    }

    #[test]
    fn transaction_can_be_created() {
        let tx = transaction();

        assert_eq!(tx.version, 1);
        assert_eq!(tx.fee, 100);
    }

    #[test]
    fn transaction_id_is_32_bytes() {
        let tx = transaction();

        assert_eq!(tx.id().len(), 32);
    }

    #[test]
    fn transaction_can_be_signed() {
        let mut tx = transaction();

        tx.sign_input(0, &secret_key())
            .expect("signing should succeed");

        assert_eq!(tx.inputs[0].public_key.len(), 33);
        assert_eq!(tx.inputs[0].signature.len(), 64);
    }

    #[test]
    fn valid_signature_is_accepted() {
        let mut tx = transaction();

        tx.sign_input(0, &secret_key())
            .expect("signing should succeed");

        assert!(tx.verify_input(0).expect("verification should succeed"));
    }

    #[test]
    fn unsigned_transaction_is_rejected() {
        let tx = transaction();

        assert!(!tx.validate_signatures());
    }

    #[test]
    fn wrong_key_signature_is_rejected() {
        let mut tx = transaction();

        tx.sign_input(0, &secret_key())
            .expect("signing should succeed");

        let original_public_key = tx.inputs[0].public_key.clone();

        let wrong_key = second_secret_key();

        let secp = Secp256k1::new();

        let hash = tx
            .signing_hash(0, &original_public_key)
            .expect("hash should succeed");

        let message = Message::from_digest(hash);

        let wrong_signature = secp.sign_ecdsa(&message, &wrong_key);

        tx.inputs[0].signature = wrong_signature.serialize_compact().to_vec();

        assert!(!tx.validate_signatures());
    }

    #[test]
    fn changing_output_invalidates_signature() {
        let mut tx = transaction();

        tx.sign_input(0, &secret_key())
            .expect("signing should succeed");

        assert!(tx.validate_signatures());

        tx.outputs[0].amount = 901;

        assert!(!tx.validate_signatures());
    }

    #[test]
    fn changing_fee_invalidates_signature() {
        let mut tx = transaction();

        tx.sign_input(0, &secret_key())
            .expect("signing should succeed");

        assert!(tx.validate_signatures());

        tx.fee = 101;

        assert!(!tx.validate_signatures());
    }

    #[test]
    fn changing_previous_output_invalidates_signature() {
        let mut tx = transaction();

        tx.sign_input(0, &secret_key())
            .expect("signing should succeed");

        assert!(tx.validate_signatures());

        tx.inputs[0].previous_output = [9u8; 32];

        assert!(!tx.validate_signatures());
    }

    #[test]
    fn changing_public_key_invalidates_signature() {
        let mut tx = transaction();

        tx.sign_input(0, &secret_key())
            .expect("signing should succeed");

        assert!(tx.validate_signatures());

        tx.inputs[0].public_key = vec![3u8; 33];

        assert!(!tx.validate_signatures());
    }

    #[test]
    fn changing_signature_invalidates_transaction() {
        let mut tx = transaction();

        tx.sign_input(0, &secret_key())
            .expect("signing should succeed");

        assert!(tx.validate_signatures());

        tx.inputs[0].signature[0] ^= 1;

        assert!(!tx.validate_signatures());
    }

    #[test]
    fn transaction_id_changes_after_signature() {
        let tx1 = transaction();

        let mut tx2 = transaction();

        let before = tx1.id();

        tx2.sign_input(0, &secret_key())
            .expect("signing should succeed");

        let after = tx2.id();

        assert_ne!(before, after);
    }

    #[test]
    fn all_signatures_can_be_verified() {
        let mut tx = Transaction::new(1, vec![input(), second_input()], vec![output()], 100);

        tx.sign_input(0, &secret_key())
            .expect("first signature should work");

        tx.sign_input(1, &second_secret_key())
            .expect("second signature should work");

        assert!(tx.verify_input(0).expect("first verification should work"));

        assert!(tx.verify_input(1).expect("second verification should work"));

        assert!(tx.validate_signatures());
    }

    // ============================================================
    // COINBASE TESTS
    // ============================================================

    #[test]
    fn coinbase_transaction_can_be_created() {
        let tx = Transaction::coinbase(500, vec![1u8; 33]).expect("coinbase should be valid");

        assert!(tx.is_coinbase());
        assert!(tx.validate_coinbase());
    }

    #[test]
    fn coinbase_cannot_be_signed() {
        let mut tx = Transaction::coinbase(500, vec![1u8; 33]).expect("coinbase should be valid");

        assert!(tx.sign_input(0, &secret_key()).is_err());
    }

    #[test]
    fn zero_coinbase_reward_is_rejected() {
        assert!(Transaction::coinbase(0, vec![1u8; 33],).is_err());
    }

    #[test]
    fn empty_coinbase_recipient_is_rejected() {
        assert!(Transaction::coinbase(500, Vec::new(),).is_err());
    }

    #[test]
    fn coinbase_with_fee_is_rejected() {
        let tx = Transaction {
            version: 1,
            inputs: Vec::new(),
            outputs: vec![TransactionOutput {
                amount: 500,
                recipient: vec![1u8; 33],
            }],
            fee: 1,
        };

        assert!(!tx.validate_coinbase());
    }

    #[test]
    fn coinbase_with_multiple_outputs_is_rejected() {
        let tx = Transaction {
            version: 1,
            inputs: Vec::new(),
            outputs: vec![
                TransactionOutput {
                    amount: 500,
                    recipient: vec![1u8; 33],
                },
                TransactionOutput {
                    amount: 100,
                    recipient: vec![2u8; 33],
                },
            ],
            fee: 0,
        };

        assert!(!tx.validate_coinbase());
    }

    #[test]
    fn empty_inputs_are_not_a_valid_normal_transaction() {
        let tx = Transaction {
            version: 1,
            inputs: Vec::new(),
            outputs: vec![TransactionOutput {
                amount: 500,
                recipient: vec![1u8; 33],
            }],
            fee: 0,
        };

        assert!(!tx.validate_basic());
        assert!(tx.is_coinbase());
        assert!(tx.validate_coinbase());
    }
}
