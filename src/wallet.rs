use crate::transaction::Transaction;
use getrandom::getrandom;
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use sha2::{Digest, Sha256};

/// طول کلید خصوصی Secp256k1
const SECRET_KEY_SIZE: usize = 32;

/// نسخه فعلی فرمت Address
const ADDRESS_VERSION: u8 = 1;

/// NIO Wallet
///
/// این ساختار کلید خصوصی را در حافظه نگه می‌دارد.
/// در مراحل بعدی ذخیره‌سازی امن و رمزگذاری Wallet
/// جداگانه اضافه خواهد شد.
#[derive(Debug)]
pub struct Wallet {
    secret_key: SecretKey,
}

impl Wallet {
    // ============================================================
    // CREATE WALLET
    // ============================================================

    /// ایجاد Wallet جدید با منبع تصادفی سیستم‌عامل.
    pub fn new() -> Result<Self, String> {
        let mut bytes = [0u8; SECRET_KEY_SIZE];

        getrandom(&mut bytes).map_err(|_| "failed to obtain secure random bytes".to_string())?;

        let secret_key = SecretKey::from_slice(&bytes)
            .map_err(|_| "generated invalid secret key".to_string())?;

        Ok(Self { secret_key })
    }

    /// ایجاد Wallet از Private Key موجود.
    ///
    /// برای recovery/import و تست‌ها استفاده می‌شود.
    pub fn from_secret_key(secret_key: SecretKey) -> Self {
        Self { secret_key }
    }

    // ============================================================
    // PRIVATE KEY
    // ============================================================

    /// دسترسی کنترل‌شده به Private Key.
    ///
    /// در UI یا log نباید این مقدار نمایش داده شود.
    pub fn secret_key(&self) -> &SecretKey {
        &self.secret_key
    }

    // ============================================================
    // PUBLIC KEY
    // ============================================================

    /// استخراج Public Key فشرده Secp256k1.
    pub fn public_key(&self) -> PublicKey {
        let secp = Secp256k1::new();

        PublicKey::from_secret_key(&secp, &self.secret_key)
    }

    /// Public Key به صورت bytes.
    pub fn public_key_bytes(&self) -> Vec<u8> {
        self.public_key().serialize().to_vec()
    }

    // ============================================================
    // ADDRESS
    // ============================================================

    /// ساخت Address پایه NIO.
    ///
    /// فعلاً:
    ///
    /// SHA256(version || compressed_public_key)
    ///
    /// استفاده می‌شود.
    ///
    /// فرمت نهایی قابل نمایش:
    ///
    /// NIO1 + hexadecimal payload
    ///
    /// در مرحله Address Encoding می‌توانیم Base58Check
    /// یا فرمت نهایی اختصاصی NIO را اضافه کنیم.
    pub fn address(&self) -> String {
        let public_key = self.public_key_bytes();

        let mut hasher = Sha256::new();

        hasher.update([ADDRESS_VERSION]);
        hasher.update(&public_key);

        let digest = hasher.finalize();

        let mut address = String::from("NIO1");

        for byte in digest {
            address.push_str(&format!("{:02x}", byte));
        }

        address
    }

    // ============================================================
    // SIGN TRANSACTION
    // ============================================================

    /// امضای یکی از ورودی‌های Transaction.
    pub fn sign_transaction(
        &self,
        transaction: &mut Transaction,
        input_index: usize,
    ) -> Result<(), String> {
        transaction.sign_input(input_index, &self.secret_key)
    }

    /// امضای تمام ورودی‌های Transaction.
    pub fn sign_transaction_all(&self, transaction: &mut Transaction) -> Result<(), String> {
        for index in 0..transaction.inputs.len() {
            self.sign_transaction(transaction, index)?;
        }

        Ok(())
    }

    // ============================================================
    // BALANCE
    // ============================================================

    /// محاسبه موجودی Wallet از UTXO Set.
    ///
    /// در این مرحله recipient باید Public Key Wallet باشد.
    pub fn balance(&self, utxo_set: &crate::utxo::UtxoSet) -> u64 {
        let public_key = self.public_key_bytes();

        utxo_set
            .iter()
            .filter(|utxo| utxo.recipient == public_key)
            .fold(0u64, |total, utxo| total.saturating_add(utxo.amount))
    }
}

// ================================================================
// TESTS
// ================================================================

#[cfg(test)]
mod tests {
    use super::*;

    use crate::transaction::{Transaction, TransactionInput, TransactionOutput};

    use crate::utxo::{Utxo, UtxoId, UtxoSet};

    fn test_secret_key() -> SecretKey {
        SecretKey::from_slice(&[1u8; 32]).expect("test secret key must be valid")
    }

    fn make_transaction() -> Transaction {
        Transaction::new(
            1,
            vec![TransactionInput {
                previous_output: [7u8; 32],
                output_index: 0,
                public_key: Vec::new(),
                signature: Vec::new(),
            }],
            vec![TransactionOutput {
                amount: 900,
                recipient: vec![2u8; 33],
            }],
            100,
        )
    }

    // ------------------------------------------------------------
    // WALLET CREATION
    // ------------------------------------------------------------

    #[test]
    fn wallet_can_be_created() {
        let wallet = Wallet::new().expect("wallet creation should succeed");

        assert_eq!(wallet.secret_key().secret_bytes().len(), 32);
    }

    #[test]
    fn two_wallets_have_different_keys() {
        let wallet1 = Wallet::new().expect("wallet 1 should be created");

        let wallet2 = Wallet::new().expect("wallet 2 should be created");

        assert_ne!(
            wallet1.secret_key().secret_bytes(),
            wallet2.secret_key().secret_bytes()
        );
    }

    // ------------------------------------------------------------
    // PUBLIC KEY
    // ------------------------------------------------------------

    #[test]
    fn public_key_is_33_bytes() {
        let wallet = Wallet::from_secret_key(test_secret_key());

        assert_eq!(wallet.public_key_bytes().len(), 33);
    }

    #[test]
    fn same_secret_key_produces_same_public_key() {
        let wallet1 = Wallet::from_secret_key(test_secret_key());

        let wallet2 = Wallet::from_secret_key(test_secret_key());

        assert_eq!(wallet1.public_key_bytes(), wallet2.public_key_bytes());
    }

    // ------------------------------------------------------------
    // ADDRESS
    // ------------------------------------------------------------

    #[test]
    fn address_starts_with_nio_prefix() {
        let wallet = Wallet::from_secret_key(test_secret_key());

        assert!(wallet.address().starts_with("NIO1"));
    }

    #[test]
    fn same_key_produces_same_address() {
        let wallet1 = Wallet::from_secret_key(test_secret_key());

        let wallet2 = Wallet::from_secret_key(test_secret_key());

        assert_eq!(wallet1.address(), wallet2.address());
    }

    #[test]
    fn different_keys_produce_different_addresses() {
        let wallet1 = Wallet::from_secret_key(test_secret_key());

        let wallet2 = Wallet::new().expect("wallet should be created");

        assert_ne!(wallet1.address(), wallet2.address());
    }

    // ------------------------------------------------------------
    // TRANSACTION SIGNING
    // ------------------------------------------------------------

    #[test]
    fn wallet_can_sign_transaction() {
        let wallet = Wallet::from_secret_key(test_secret_key());

        let mut tx = make_transaction();

        wallet
            .sign_transaction(&mut tx, 0)
            .expect("transaction should be signed");

        assert!(tx.verify_input(0).expect("verification should work"));
    }

    #[test]
    fn wallet_can_sign_all_inputs() {
        let wallet = Wallet::from_secret_key(test_secret_key());

        let mut tx = Transaction::new(
            1,
            vec![
                TransactionInput {
                    previous_output: [7u8; 32],
                    output_index: 0,
                    public_key: Vec::new(),
                    signature: Vec::new(),
                },
                TransactionInput {
                    previous_output: [8u8; 32],
                    output_index: 0,
                    public_key: Vec::new(),
                    signature: Vec::new(),
                },
            ],
            vec![TransactionOutput {
                amount: 1800,
                recipient: vec![2u8; 33],
            }],
            200,
        );

        wallet
            .sign_transaction_all(&mut tx)
            .expect("all inputs should be signed");

        assert!(tx
            .verify_input(0)
            .expect("first input verification should work"));

        assert!(tx
            .verify_input(1)
            .expect("second input verification should work"));
    }

    // ------------------------------------------------------------
    // BALANCE
    // ------------------------------------------------------------

    #[test]
    fn wallet_balance_is_calculated_from_utxos() {
        let wallet = Wallet::from_secret_key(test_secret_key());

        let recipient = wallet.public_key_bytes();

        let mut set = UtxoSet::new();

        set.insert(Utxo {
            id: UtxoId {
                transaction_id: [1u8; 32],
                output_index: 0,
            },
            amount: 400,
            recipient: recipient.clone(),
        })
        .expect("UTXO insertion should succeed");

        set.insert(Utxo {
            id: UtxoId {
                transaction_id: [2u8; 32],
                output_index: 0,
            },
            amount: 600,
            recipient,
        })
        .expect("UTXO insertion should succeed");

        assert_eq!(wallet.balance(&set), 1000);
    }

    #[test]
    fn wallet_does_not_count_other_addresses() {
        let wallet = Wallet::from_secret_key(test_secret_key());

        let mut set = UtxoSet::new();

        set.insert(Utxo {
            id: UtxoId {
                transaction_id: [1u8; 32],
                output_index: 0,
            },
            amount: 1000,
            recipient: vec![9u8; 33],
        })
        .expect("UTXO insertion should succeed");

        assert_eq!(wallet.balance(&set), 0);
    }
}
