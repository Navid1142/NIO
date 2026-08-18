use nio_blockchain::economy::{HALVING_INTERVAL_BLOCKS, MINING_ALLOCATION_NIO, UNITS_PER_NIO};

const MAX_HALVINGS: u32 = 64;

/// Calculate the total number of blocks in one halving era.
fn blocks_per_era() -> u64 {
    HALVING_INTERVAL_BLOCKS
}

/// Calculate total mining produced by a reward
/// across one complete halving era.
fn era_emission(reward_units: u64) -> u128 {
    (reward_units as u128) * (blocks_per_era() as u128)
}

/// Calculate the theoretical total emission
/// for a given initial reward.
///
/// Every era halves the reward.
/// Integer division is used because NIO works
/// in its smallest unit.
fn total_emission(initial_reward_units: u64) -> u128 {
    let mut reward = initial_reward_units;

    let mut total: u128 = 0;

    for _ in 0..MAX_HALVINGS {
        if reward == 0 {
            break;
        }

        total += era_emission(reward);

        reward /= 2;
    }

    total
}

/// Find the largest initial reward that does
/// not exceed the mining allocation.
fn find_max_safe_initial_reward(cap_units: u64) -> u64 {
    let mut low: u64 = 0;
    let mut high: u64 = cap_units;

    while low < high {
        let mid = low + (high - low).div_ceil(2);

        let emission = total_emission(mid);

        if emission <= cap_units as u128 {
            low = mid;
        } else {
            high = mid - 1;
        }
    }

    low
}

fn main() {
    let cap_units = MINING_ALLOCATION_NIO * UNITS_PER_NIO;

    println!("================================");
    println!("       NIO SUPPLY ANALYZER");
    println!("================================");

    println!("Mining allocation : {} NIO", MINING_ALLOCATION_NIO);

    println!("Mining cap units  : {}", cap_units);

    println!("Block time        : {} seconds", 60);

    println!("Halving interval  : {} blocks", HALVING_INTERVAL_BLOCKS);

    println!("================================");

    let max_reward = find_max_safe_initial_reward(cap_units);

    let total = total_emission(max_reward);

    let remaining = (cap_units as u128).saturating_sub(total);

    println!("Maximum safe initial reward:");

    println!("{} units", max_reward);

    println!("{:.6} NIO", max_reward as f64 / UNITS_PER_NIO as f64);

    println!("--------------------------------");

    println!("Theoretical total emission:");

    println!("{} units", total);

    println!("{:.6} NIO", total as f64 / UNITS_PER_NIO as f64);

    println!("--------------------------------");

    println!("Remaining mining allocation:");

    println!("{} units", remaining);

    println!("{:.6} NIO", remaining as f64 / UNITS_PER_NIO as f64);

    println!("================================");

    if total <= cap_units as u128 {
        println!("STATUS: HARD CAP PASSED");
    } else {
        println!("STATUS: HARD CAP FAILED");
    }

    println!("================================");

    println!("HALVING SCHEDULE");

    println!("================================");

    let mut reward = max_reward;

    for era in 0..MAX_HALVINGS {
        if reward == 0 {
            break;
        }

        let emission = era_emission(reward);

        println!(
            "Era {:>2} | Reward {:>12} units | {:.6} NIO | Emission {:>18} units",
            era,
            reward,
            reward as f64 / UNITS_PER_NIO as f64,
            emission
        );

        reward /= 2;
    }

    println!("================================");
}
