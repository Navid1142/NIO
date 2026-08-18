use crate::economy::Economy;
use crate::reserve::ProjectReserve;

pub const TREASURY_ADDRESS_LENGTH: usize = 33;

/// آدرس عمومی خزانه پروژه.
///
/// این ساختار فقط Public Address را نگهداری می‌کند.
/// Private Key هرگز داخل این ساختار ذخیره نمی‌شود.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreasuryAddress {
    bytes: [u8; TREASURY_ADDRESS_LENGTH],
}

impl TreasuryAddress {
    /// ایجاد Treasury Address از یک Public Key/Address معتبر.
    pub fn new(bytes: [u8; TREASURY_ADDRESS_LENGTH]) -> Result<Self, String> {
        if bytes == [0u8; TREASURY_ADDRESS_LENGTH] {
            return Err("treasury address cannot be all zeros".to_string());
        }

        Ok(Self { bytes })
    }

    /// دسترسی فقط به Public Address.
    pub const fn as_bytes(&self) -> &[u8; TREASURY_ADDRESS_LENGTH] {
        &self.bytes
    }

    /// بررسی اعتبار آدرس.
    pub fn is_valid(&self) -> bool {
        self.bytes != [0u8; TREASURY_ADDRESS_LENGTH]
    }
}

/// تخصیص ذخیره پروژه به Treasury.
///
/// این ساختار فقط مشخص می‌کند که چه مقدار Reserve
/// باید متعلق به Treasury باشد.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreasuryAllocation {
    address: TreasuryAddress,
    amount: u64,
}

impl TreasuryAllocation {
    /// ایجاد تخصیص رسمی Treasury.
    ///
    /// مقدار همیشه باید دقیقاً برابر Project Reserve باشد.
    pub fn new(address: TreasuryAddress) -> Result<Self, String> {
        let reserve = ProjectReserve::new()?;

        if !reserve.is_valid() {
            return Err("invalid project reserve policy".to_string());
        }

        let amount = reserve.amount();

        if amount != Economy::project_reserve() {
            return Err("treasury amount does not match project reserve".to_string());
        }

        Ok(Self { address, amount })
    }

    pub const fn address(&self) -> &TreasuryAddress {
        &self.address
    }

    pub const fn amount(&self) -> u64 {
        self.amount
    }

    /// Treasury باید دقیقاً 100 NIO باشد.
    pub fn is_valid(&self) -> bool {
        self.address.is_valid()
            && self.amount == Economy::project_reserve()
            && self.amount + Economy::mining_cap() == Economy::total_supply()
    }

    /// Treasury هرگز بخشی از Mining Supply نیست.
    pub fn is_not_mining_supply(&self) -> bool {
        self.amount == Economy::project_reserve() && self.amount != Economy::mining_cap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_address() -> TreasuryAddress {
        TreasuryAddress::new([7u8; TREASURY_ADDRESS_LENGTH])
            .expect("test treasury address must be valid")
    }

    #[test]
    fn treasury_address_can_be_created() {
        let address = valid_address();

        assert!(address.is_valid());
        assert_eq!(address.as_bytes(), &[7u8; TREASURY_ADDRESS_LENGTH]);
    }

    #[test]
    fn zero_address_is_rejected() {
        assert!(TreasuryAddress::new([0u8; TREASURY_ADDRESS_LENGTH]).is_err());
    }

    #[test]
    fn treasury_allocation_is_exactly_project_reserve() {
        let allocation =
            TreasuryAllocation::new(valid_address()).expect("allocation should succeed");

        assert_eq!(allocation.amount(), Economy::project_reserve());
    }

    #[test]
    fn treasury_allocation_is_valid() {
        let allocation =
            TreasuryAllocation::new(valid_address()).expect("allocation should succeed");

        assert!(allocation.is_valid());
    }

    #[test]
    fn treasury_is_not_mining_supply() {
        let allocation =
            TreasuryAllocation::new(valid_address()).expect("allocation should succeed");

        assert!(allocation.is_not_mining_supply());
        assert_ne!(allocation.amount(), Economy::mining_cap());
    }

    #[test]
    fn treasury_plus_mining_equals_total_supply() {
        let allocation =
            TreasuryAllocation::new(valid_address()).expect("allocation should succeed");

        assert_eq!(
            allocation.amount() + Economy::mining_cap(),
            Economy::total_supply()
        );
    }

    #[test]
    fn treasury_amount_cannot_be_zero() {
        let allocation =
            TreasuryAllocation::new(valid_address()).expect("allocation should succeed");

        assert!(allocation.amount() > 0);
    }
}
