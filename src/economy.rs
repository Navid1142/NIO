/// NIO BLOCKCHAIN
/// Economy / Consensus Monetary Policy
///
/// Total Supply      = 1,000 NIO
/// Project Reserve   =   100 NIO
/// Mining Allocation =   900 NIO
///
/// Smallest unit:
/// 1 NIO = 100,000,000,000,000 units (14 decimals)
///
/// Block time:
/// 60 seconds
///
/// Total planned blocks:
/// 450,000,000
///
/// Halving interval:
/// 45,000,000 blocks
///
/// Number of halving eras:
/// 10
///
/// IMPORTANT:
/// All consensus calculations use integer units.
/// The mining hard cap can never be exceeded.
pub const DECIMALS: u32 = 14;

pub const UNITS_PER_NIO: u64 = 100_000_000_000_000;

// ============================================================
// SUPPLY
// ============================================================

pub const TOTAL_SUPPLY_NIO: u64 = 1_000;

pub const MINING_ALLOCATION_NIO: u64 = 900;

pub const PROJECT_RESERVE_NIO: u64 = 100;

pub const TOTAL_SUPPLY_UNITS: u64 = TOTAL_SUPPLY_NIO * UNITS_PER_NIO;

pub const MINING_ALLOCATION_UNITS: u64 = MINING_ALLOCATION_NIO * UNITS_PER_NIO;

pub const PROJECT_RESERVE_UNITS: u64 = PROJECT_RESERVE_NIO * UNITS_PER_NIO;

// ============================================================
// BLOCK PARAMETERS
// ============================================================

pub const TARGET_BLOCK_TIME_SECONDS: u64 = 60;

pub const TOTAL_BLOCKS: u64 = 450_000_000;

pub const HALVING_INTERVAL_BLOCKS: u64 = 2_102_400;

pub const HALVING_ERAS: u64 = 10;

// ============================================================
// INITIAL REWARD
// ============================================================

/// Initial theoretical mining reward.
///
/// 1,000,977,517 units
///
/// = 0.00001000977517 NIO
///
/// The reward schedule is halved every era.
pub const INITIAL_REWARD_UNITS: u64 = 1_000_977_517;

// ============================================================
// ECONOMY
// ============================================================

pub struct Economy;

impl Economy {
    /// Maximum amount that can ever be created through mining.
    pub const fn mining_cap() -> u64 {
        MINING_ALLOCATION_UNITS
    }

    /// Absolute maximum supply.
    pub const fn total_supply() -> u64 {
        TOTAL_SUPPLY_UNITS
    }

    /// Permanent project reserve.
    pub const fn project_reserve() -> u64 {
        PROJECT_RESERVE_UNITS
    }

    /// Returns the halving era for a block height.
    pub const fn halving_era(height: u64) -> u64 {
        height / HALVING_INTERVAL_BLOCKS
    }

    /// Returns the theoretical reward before the remaining
    /// mining-cap limit is applied.
    ///
    /// Integer division is used intentionally.
    pub const fn base_reward_at_height(height: u64) -> u64 {
        let era = Self::halving_era(height);

        if era >= HALVING_ERAS {
            return 0;
        }

        INITIAL_REWARD_UNITS >> era
    }

    /// Returns the actual reward allowed by consensus.
    ///
    /// The reward can NEVER exceed the remaining mining supply.
    pub const fn block_reward(height: u64, current_mining_supply: u64) -> u64 {
        if current_mining_supply >= MINING_ALLOCATION_UNITS {
            return 0;
        }

        let theoretical = Self::base_reward_at_height(height);

        if theoretical == 0 {
            return 0;
        }

        let remaining = MINING_ALLOCATION_UNITS - current_mining_supply;

        if theoretical > remaining {
            remaining
        } else {
            theoretical
        }
    }

    /// Checks whether a claimed mining reward is valid.
    pub const fn is_valid_mining_reward(
        height: u64,
        current_mining_supply: u64,
        claimed_reward: u64,
    ) -> bool {
        claimed_reward == Self::block_reward(height, current_mining_supply)
    }

    /// Returns remaining mining allocation.
    pub const fn remaining_mining(current_mining_supply: u64) -> u64 {
        MINING_ALLOCATION_UNITS.saturating_sub(current_mining_supply)
    }

    /// Checks whether mining supply is inside the hard cap.
    pub const fn is_supply_valid(current_mining_supply: u64) -> bool {
        current_mining_supply <= MINING_ALLOCATION_UNITS
    }

    /// Checks token allocation.
    pub const fn allocations_are_valid() -> bool {
        MINING_ALLOCATION_UNITS + PROJECT_RESERVE_UNITS == TOTAL_SUPPLY_UNITS
    }

    /// Checks whether the configured monetary policy is valid.
    pub const fn policy_is_valid() -> bool {
        DECIMALS == 14
            && TOTAL_SUPPLY_NIO == 1_000
            && MINING_ALLOCATION_NIO == 900
            && PROJECT_RESERVE_NIO == 100
            && TARGET_BLOCK_TIME_SECONDS == 60
            && TOTAL_BLOCKS == 450_000_000
            && HALVING_INTERVAL_BLOCKS == 2_102_400
            && HALVING_ERAS == 10
            && Self::allocations_are_valid()
    }
}

// ============================================================
// TESTS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimals_are_14() {
        assert_eq!(DECIMALS, 14);
    }

    #[test]
    fn one_nio_has_correct_units() {
        assert_eq!(UNITS_PER_NIO, 100_000_000_000_000);
    }

    #[test]
    fn total_supply_is_1000_nio() {
        assert_eq!(TOTAL_SUPPLY_UNITS, 1000 * UNITS_PER_NIO);
    }

    #[test]
    fn mining_allocation_is_900_nio() {
        assert_eq!(MINING_ALLOCATION_UNITS, 900 * UNITS_PER_NIO);
    }

    #[test]
    fn project_reserve_is_100_nio() {
        assert_eq!(PROJECT_RESERVE_UNITS, 100 * UNITS_PER_NIO);
    }

    #[test]
    fn allocations_equal_total_supply() {
        assert!(Economy::allocations_are_valid());

        assert_eq!(
            MINING_ALLOCATION_UNITS + PROJECT_RESERVE_UNITS,
            TOTAL_SUPPLY_UNITS
        );
    }

    #[test]
    fn block_time_is_60_seconds() {
        assert_eq!(TARGET_BLOCK_TIME_SECONDS, 60);
    }

    #[test]
    fn total_blocks_are_correct() {
        assert_eq!(TOTAL_BLOCKS, 450_000_000);
    }

    #[test]
    fn halving_interval_is_correct() {
        assert_eq!(HALVING_INTERVAL_BLOCKS, 2_102_400);
    }

    #[test]
    fn halving_eras_are_correct() {
        assert_eq!(HALVING_ERAS, 10);
    }

    #[test]
    fn initial_reward_is_correct() {
        assert_eq!(INITIAL_REWARD_UNITS, 1_000_977_517);

        assert_eq!(Economy::block_reward(0, 0), INITIAL_REWARD_UNITS);
    }

    #[test]
    fn first_halving_is_correct() {
        assert_eq!(
            Economy::block_reward(HALVING_INTERVAL_BLOCKS, 0,),
            INITIAL_REWARD_UNITS / 2
        );
    }

    #[test]
    fn second_halving_is_correct() {
        assert_eq!(
            Economy::block_reward(HALVING_INTERVAL_BLOCKS * 2, 0,),
            INITIAL_REWARD_UNITS / 4
        );
    }

    #[test]
    fn halving_era_zero() {
        assert_eq!(Economy::halving_era(0), 0);

        assert_eq!(Economy::halving_era(HALVING_INTERVAL_BLOCKS - 1), 0);
    }

    #[test]
    fn halving_boundary_is_correct() {
        assert_eq!(Economy::halving_era(HALVING_INTERVAL_BLOCKS), 1);
    }

    #[test]
    fn reward_becomes_zero_after_final_era() {
        assert_eq!(Economy::block_reward(TOTAL_BLOCKS, 0,), 0);
    }

    #[test]
    fn reward_is_zero_at_mining_cap() {
        assert_eq!(Economy::block_reward(0, MINING_ALLOCATION_UNITS), 0);
    }

    #[test]
    fn reward_never_exceeds_remaining_supply() {
        let remaining = 10;

        let current = MINING_ALLOCATION_UNITS - remaining;

        assert_eq!(Economy::block_reward(0, current), remaining);
    }

    #[test]
    fn excessive_reward_is_rejected() {
        assert!(!Economy::is_valid_mining_reward(
            0,
            0,
            INITIAL_REWARD_UNITS + 1
        ));
    }

    #[test]
    fn correct_reward_is_accepted() {
        assert!(Economy::is_valid_mining_reward(0, 0, INITIAL_REWARD_UNITS));
    }

    #[test]
    fn zero_reward_is_rejected_before_cap() {
        assert!(!Economy::is_valid_mining_reward(0, 0, 0));
    }

    #[test]
    fn zero_reward_is_accepted_when_cap_is_reached() {
        assert!(Economy::is_valid_mining_reward(
            0,
            MINING_ALLOCATION_UNITS,
            0
        ));
    }

    #[test]
    fn remaining_mining_is_correct() {
        assert_eq!(Economy::remaining_mining(0), MINING_ALLOCATION_UNITS);

        assert_eq!(Economy::remaining_mining(MINING_ALLOCATION_UNITS), 0);
    }

    #[test]
    fn supply_above_cap_is_invalid() {
        assert!(!Economy::is_supply_valid(MINING_ALLOCATION_UNITS + 1));
    }

    #[test]
    fn policy_is_valid() {
        assert!(Economy::policy_is_valid());
    }
}
