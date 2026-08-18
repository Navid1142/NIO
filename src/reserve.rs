use crate::economy::Economy;

/// مقدار ذخیره اختصاصی پروژه.
///
/// این مقدار بخشی از کل عرضه است اما از Mining Supply جداست.
/// 100 NIO = Project Reserve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectReserve {
    amount: u64,
}

impl ProjectReserve {
    /// ایجاد ذخیره پروژه با مقدار تعریف‌شده در Economy.
    pub fn new() -> Result<Self, String> {
        let amount = Economy::project_reserve();

        if amount == 0 {
            return Err("project reserve cannot be zero".to_string());
        }

        Ok(Self { amount })
    }

    /// مقدار ذخیره پروژه بر حسب کوچک‌ترین واحد NIO.
    pub const fn amount(&self) -> u64 {
        self.amount
    }

    /// بررسی اینکه مقدار دقیقاً برابر Reserve تعریف‌شده در
    /// Monetary Policy باشد.
    pub fn is_valid(&self) -> bool {
        self.amount == Economy::project_reserve()
    }

    /// ذخیره پروژه نباید بخشی از Mining Supply باشد.
    pub fn is_separate_from_mining(&self, mining_supply: u64) -> bool {
        mining_supply <= Economy::mining_cap() && self.amount == Economy::project_reserve()
    }

    /// بررسی اینکه Reserve + Mining Cap دقیقاً کل عرضه را تشکیل می‌دهد.
    pub fn allocation_is_valid(&self) -> bool {
        self.amount.checked_add(Economy::mining_cap()) == Some(Economy::total_supply())
    }
}

impl Default for ProjectReserve {
    fn default() -> Self {
        Self::new().expect("project reserve policy must be valid")
    }
}

// ================================================================
// TESTS
// ================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserve_is_exactly_100_nio() {
        let reserve = ProjectReserve::new().expect("reserve should be created");

        assert_eq!(reserve.amount(), Economy::project_reserve());
    }

    #[test]
    fn reserve_is_valid() {
        let reserve = ProjectReserve::new().expect("reserve should be created");

        assert!(reserve.is_valid());
    }

    #[test]
    fn reserve_is_separate_from_mining() {
        let reserve = ProjectReserve::new().expect("reserve should be created");

        assert!(reserve.is_separate_from_mining(0));
        assert!(reserve.is_separate_from_mining(Economy::mining_cap()));
    }

    #[test]
    fn reserve_and_mining_equal_total_supply() {
        let reserve = ProjectReserve::new().expect("reserve should be created");

        assert!(reserve.allocation_is_valid());
    }

    #[test]
    fn reserve_is_not_mining_supply() {
        let reserve = ProjectReserve::new().expect("reserve should be created");

        assert_ne!(reserve.amount(), Economy::mining_cap());
    }

    #[test]
    fn reserve_cannot_be_zero() {
        let reserve = ProjectReserve::new().expect("reserve should be created");

        assert!(reserve.amount() > 0);
    }
}
