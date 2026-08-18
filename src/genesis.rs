use crate::economy::{Economy, MINING_ALLOCATION_UNITS, PROJECT_RESERVE_UNITS, TOTAL_SUPPLY_UNITS};

/// Immutable Genesis allocation.
///
/// NIO initial supply:
///
/// Total Supply      = 1,000 NIO
/// Project Reserve   =   100 NIO
/// Mining Allocation =   900 NIO
///
/// IMPORTANT:
/// The project reserve is NOT mining supply.
/// It must never be added to the mining reward calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenesisState {
    total_supply: u64,
    project_reserve: u64,
    mining_allocation: u64,
}

impl GenesisState {
    /// Creates the canonical NIO genesis state.
    pub const fn new() -> Self {
        Self {
            total_supply: TOTAL_SUPPLY_UNITS,
            project_reserve: PROJECT_RESERVE_UNITS,
            mining_allocation: MINING_ALLOCATION_UNITS,
        }
    }

    /// Total maximum NIO supply.
    pub const fn total_supply(&self) -> u64 {
        self.total_supply
    }

    /// Permanent project reserve.
    pub const fn project_reserve(&self) -> u64 {
        self.project_reserve
    }

    /// Maximum amount available through mining.
    pub const fn mining_allocation(&self) -> u64 {
        self.mining_allocation
    }

    /// Verifies the canonical Genesis allocation.
    pub const fn is_valid(&self) -> bool {
        self.total_supply == TOTAL_SUPPLY_UNITS
            && self.project_reserve == PROJECT_RESERVE_UNITS
            && self.mining_allocation == MINING_ALLOCATION_UNITS
            && self.project_reserve + self.mining_allocation == self.total_supply
            && Economy::allocations_are_valid()
    }

    /// Returns true when the specified amount belongs to
    /// the mining allocation.
    pub const fn is_mining_amount(&self, amount: u64) -> bool {
        amount <= self.mining_allocation
    }

    /// Returns true when the specified amount belongs to
    /// the project reserve allocation.
    pub const fn is_reserve_amount(&self, amount: u64) -> bool {
        amount <= self.project_reserve
    }

    /// Genesis mining supply always starts at zero.
    pub const fn initial_mining_supply(&self) -> u64 {
        0
    }

    /// Genesis cannot create an additional mining allocation.
    pub const fn mining_cap(&self) -> u64 {
        self.mining_allocation
    }

    /// Checks that the mining cap does not include the project reserve.
    pub const fn reserve_is_outside_mining(&self) -> bool {
        self.mining_allocation + self.project_reserve == self.total_supply
            && self.mining_allocation != self.total_supply
    }
}

impl Default for GenesisState {
    fn default() -> Self {
        Self::new()
    }
}

// ================================================================
// TESTS
// ================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_is_created_correctly() {
        let genesis = GenesisState::new();

        assert_eq!(genesis.total_supply(), TOTAL_SUPPLY_UNITS);
        assert_eq!(genesis.project_reserve(), PROJECT_RESERVE_UNITS);
        assert_eq!(genesis.mining_allocation(), MINING_ALLOCATION_UNITS);
    }

    #[test]
    fn genesis_is_valid() {
        let genesis = GenesisState::new();

        assert!(genesis.is_valid());
    }

    #[test]
    fn total_supply_is_1000_nio() {
        let genesis = GenesisState::new();

        assert_eq!(
            genesis.total_supply(),
            1_000 * crate::economy::UNITS_PER_NIO
        );
    }

    #[test]
    fn project_reserve_is_100_nio() {
        let genesis = GenesisState::new();

        assert_eq!(
            genesis.project_reserve(),
            100 * crate::economy::UNITS_PER_NIO
        );
    }

    #[test]
    fn mining_allocation_is_900_nio() {
        let genesis = GenesisState::new();

        assert_eq!(
            genesis.mining_allocation(),
            900 * crate::economy::UNITS_PER_NIO
        );
    }

    #[test]
    fn reserve_plus_mining_equals_total() {
        let genesis = GenesisState::new();

        assert_eq!(
            genesis.project_reserve() + genesis.mining_allocation(),
            genesis.total_supply()
        );
    }

    #[test]
    fn genesis_mining_supply_starts_at_zero() {
        let genesis = GenesisState::new();

        assert_eq!(genesis.initial_mining_supply(), 0);
    }

    #[test]
    fn mining_cap_is_900_nio() {
        let genesis = GenesisState::new();

        assert_eq!(genesis.mining_cap(), MINING_ALLOCATION_UNITS);
    }

    #[test]
    fn reserve_is_outside_mining() {
        let genesis = GenesisState::new();

        assert!(genesis.reserve_is_outside_mining());
        assert_ne!(genesis.project_reserve(), 0);
        assert_ne!(genesis.mining_allocation(), genesis.total_supply());
    }

    #[test]
    fn reserve_amount_is_valid() {
        let genesis = GenesisState::new();

        assert!(genesis.is_reserve_amount(PROJECT_RESERVE_UNITS));
        assert!(genesis.is_reserve_amount(0));
    }

    #[test]
    fn mining_amount_is_valid() {
        let genesis = GenesisState::new();

        assert!(genesis.is_mining_amount(MINING_ALLOCATION_UNITS));
        assert!(genesis.is_mining_amount(0));
    }

    #[test]
    fn mining_cannot_exceed_genesis_allocation() {
        let genesis = GenesisState::new();

        assert!(!genesis.is_mining_amount(MINING_ALLOCATION_UNITS + 1));
    }

    #[test]
    fn reserve_cannot_exceed_genesis_allocation() {
        let genesis = GenesisState::new();

        assert!(!genesis.is_reserve_amount(PROJECT_RESERVE_UNITS + 1));
    }

    #[test]
    fn economy_policy_matches_genesis() {
        let genesis = GenesisState::new();

        assert!(Economy::policy_is_valid());
        assert!(genesis.is_valid());
    }
}
