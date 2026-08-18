use crate::transaction::Transaction;
use crate::utxo::UtxoSet;

/// سیستم محاسبه و اعتبارسنجی کارمزد تراکنش‌های NIO.
///
/// قانون اصلی:
///
/// input_total >= output_total + fee
///
/// کارمزد از موجودی ورودی تراکنش پرداخت می‌شود
/// و بعداً می‌تواند به ماینر همان بلاک برسد.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeeAccounting;

impl FeeAccounting {
    /// ایجاد سیستم Fee Accounting
    pub fn new() -> Self {
        Self
    }

    /// محاسبه کارمزد یک تراکنش.
    ///
    /// این تابع فقط مقدار fee ثبت‌شده در تراکنش
    /// را برمی‌گرداند؛ اعتبار آن جداگانه بررسی می‌شود.
    pub fn transaction_fee(transaction: &Transaction) -> u64 {
        transaction.fee
    }

    /// اعتبارسنجی کامل کارمزد یک تراکنش نسبت به UTXO Set.
    ///
    /// بررسی می‌کند که:
    ///
    /// input >= outputs + fee
    pub fn validate_transaction_fee(
        utxo_set: &UtxoSet,
        transaction: &Transaction,
    ) -> Result<(), String> {
        if transaction.is_coinbase() {
            return Err("coinbase transaction cannot contain a fee".to_string());
        }

        let input_total = utxo_set.input_value(transaction)?;

        let output_total = UtxoSet::output_value(transaction)?;

        let required = output_total
            .checked_add(transaction.fee)
            .ok_or_else(|| "output value plus fee overflow".to_string())?;

        if input_total < required {
            return Err("inputs do not cover outputs and fee".to_string());
        }

        Ok(())
    }

    /// محاسبه مجموع کارمزد تراکنش‌های یک بلاک.
    ///
    /// این تابع فقط fee را جمع می‌کند.
    /// اعتبار تراکنش‌ها باید قبل از آن بررسی شده باشد.
    pub fn total_fees(transactions: &[Transaction]) -> Result<u64, String> {
        let mut total = 0u64;

        for transaction in transactions {
            if transaction.is_coinbase() {
                if transaction.fee != 0 {
                    return Err("coinbase fee must be zero".to_string());
                }

                continue;
            }

            if !transaction.validate_basic() {
                return Err("invalid transaction while calculating fees".to_string());
            }

            total = total
                .checked_add(transaction.fee)
                .ok_or_else(|| "block fee total overflow".to_string())?;
        }

        Ok(total)
    }

    /// محاسبه مقدار قابل پرداخت به ماینر:
    ///
    /// base_reward + total_fees
    pub fn miner_payout(base_reward: u64, total_fees: u64) -> Result<u64, String> {
        base_reward
            .checked_add(total_fees)
            .ok_or_else(|| "miner payout overflow".to_string())
    }
}

impl Default for FeeAccounting {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::transaction::{Transaction, TransactionInput, TransactionOutput};

    use crate::utxo::{Utxo, UtxoId};

    fn tx_id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn make_utxo(value: u8, amount: u64) -> Utxo {
        Utxo {
            id: UtxoId {
                transaction_id: tx_id(value),
                output_index: 0,
            },
            amount,
            recipient: vec![1u8; 33],
        }
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
            recipient: vec![2u8; 33],
        }
    }

    fn make_transaction(input_value: u8, output_amount: u64, fee: u64) -> Transaction {
        Transaction::new(
            1,
            vec![make_input(input_value)],
            vec![make_output(output_amount)],
            fee,
        )
    }

    #[test]
    fn transaction_fee_is_read_correctly() {
        let tx = make_transaction(1, 900, 100);

        assert_eq!(FeeAccounting::transaction_fee(&tx), 100);
    }

    #[test]
    fn valid_fee_is_accepted() {
        let mut utxo_set = UtxoSet::new();

        utxo_set
            .insert(make_utxo(1, 1000))
            .expect("UTXO insertion should succeed");

        let tx = make_transaction(1, 900, 100);

        assert!(FeeAccounting::validate_transaction_fee(&utxo_set, &tx).is_ok());
    }

    #[test]
    fn excessive_fee_is_rejected() {
        let mut utxo_set = UtxoSet::new();

        utxo_set
            .insert(make_utxo(1, 1000))
            .expect("UTXO insertion should succeed");

        let tx = make_transaction(1, 900, 101);

        assert!(FeeAccounting::validate_transaction_fee(&utxo_set, &tx).is_err());
    }

    #[test]
    fn zero_fee_is_allowed() {
        let mut utxo_set = UtxoSet::new();

        utxo_set
            .insert(make_utxo(1, 1000))
            .expect("UTXO insertion should succeed");

        let tx = make_transaction(1, 1000, 0);

        assert!(FeeAccounting::validate_transaction_fee(&utxo_set, &tx).is_ok());
    }

    #[test]
    fn total_fees_are_calculated() {
        let tx1 = make_transaction(1, 900, 100);

        let tx2 = make_transaction(2, 450, 50);

        let transactions = vec![tx1, tx2];

        assert_eq!(
            FeeAccounting::total_fees(&transactions).expect("fee calculation should succeed"),
            150
        );
    }

    #[test]
    fn multiple_fees_are_added_safely() {
        let tx1 = make_transaction(1, 990, 10);

        let tx2 = make_transaction(2, 980, 20);

        let tx3 = make_transaction(3, 970, 30);

        let transactions = vec![tx1, tx2, tx3];

        assert_eq!(
            FeeAccounting::total_fees(&transactions).expect("fee calculation should succeed"),
            60
        );
    }

    #[test]
    fn miner_payout_adds_reward_and_fees() {
        assert_eq!(
            FeeAccounting::miner_payout(1000, 250).expect("payout calculation should succeed"),
            1250
        );
    }

    #[test]
    fn miner_payout_overflow_is_rejected() {
        assert!(FeeAccounting::miner_payout(u64::MAX, 1).is_err());
    }

    #[test]
    fn coinbase_fee_must_be_zero() {
        let coinbase =
            Transaction::coinbase(500, vec![1u8; 33]).expect("coinbase should be created");

        assert_eq!(
            FeeAccounting::total_fees(&[coinbase]).expect("coinbase should have zero fee"),
            0
        );
    }
}
