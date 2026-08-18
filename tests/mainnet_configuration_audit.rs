use nio_blockchain::economy::Economy;
use nio_blockchain::genesis::GenesisState;
use nio_blockchain::Blockchain;

#[test]
fn mainnet_genesis_policy_is_canonical() {
    let genesis = GenesisState::new();

    assert!(genesis.is_valid());

    assert_eq!(
        genesis.total_supply(),
        1_000 * nio_blockchain::economy::UNITS_PER_NIO
    );

    assert_eq!(
        genesis.project_reserve(),
        100 * nio_blockchain::economy::UNITS_PER_NIO
    );

    assert_eq!(
        genesis.mining_allocation(),
        900 * nio_blockchain::economy::UNITS_PER_NIO
    );
}

#[test]
fn mainnet_supply_components_are_exact() {
    let genesis = GenesisState::new();

    assert_eq!(
        genesis.project_reserve() + genesis.mining_allocation(),
        genesis.total_supply()
    );

    assert_eq!(
        genesis.total_supply(),
        1_000 * nio_blockchain::economy::UNITS_PER_NIO
    );
}

#[test]
fn mainnet_mining_cap_is_not_total_supply() {
    let genesis = GenesisState::new();

    assert_eq!(
        genesis.mining_cap(),
        900 * nio_blockchain::economy::UNITS_PER_NIO
    );

    assert!(genesis.mining_cap() < genesis.total_supply());
}

#[test]
fn mainnet_reserve_is_outside_mining() {
    let genesis = GenesisState::new();

    assert!(genesis.reserve_is_outside_mining());

    assert_ne!(genesis.project_reserve(), 0);

    assert_ne!(genesis.mining_allocation(), genesis.total_supply());
}

#[test]
fn mainnet_chain_starts_with_valid_genesis() {
    let chain = Blockchain::new();

    assert_eq!(chain.len(), 1);
    assert!(!chain.is_empty());

    assert_eq!(chain.genesis().header.height, 0);

    assert_eq!(chain.genesis().header.previous_hash, [0u8; 32]);

    assert_eq!(chain.genesis().header.timestamp, 0);

    assert_eq!(chain.genesis().miner_reward, 0);

    assert!(chain.genesis().transactions.is_empty());

    assert_eq!(chain.genesis().header.merkle_root, [0u8; 32]);

    assert!(chain.is_valid());
}

#[test]
fn mainnet_initial_mining_supply_is_zero() {
    let chain = Blockchain::new();

    assert_eq!(chain.mining_supply(), 0);

    assert_eq!(GenesisState::new().initial_mining_supply(), 0);
}

#[test]
fn economy_policy_matches_genesis_policy() {
    let genesis = GenesisState::new();

    assert!(Economy::policy_is_valid());

    assert!(genesis.is_valid());

    assert_eq!(Economy::total_supply(), genesis.total_supply());

    assert_eq!(Economy::project_reserve(), genesis.project_reserve());

    assert_eq!(Economy::mining_cap(), genesis.mining_allocation());
}

#[test]
fn mainnet_supply_hard_cap_is_enforced() {
    let cap = Economy::mining_cap();

    assert!(Economy::is_supply_valid(0));

    assert!(Economy::is_supply_valid(cap));

    assert!(!Economy::is_supply_valid(cap + 1));
}

#[test]
fn reward_cannot_create_supply_after_cap() {
    let cap = Economy::mining_cap();

    assert_eq!(Economy::block_reward(1, cap), 0);

    assert_eq!(Economy::block_reward(210_240, cap), 0);

    assert_eq!(Economy::block_reward(u64::MAX, cap), 0);
}

#[test]
fn reward_is_bounded_by_remaining_mining_allocation() {
    let cap = Economy::mining_cap();

    let test_heights = [
        1u64,
        2,
        210_240,
        420_480,
        630_720,
        840_960,
        1_000_000,
        u64::MAX,
    ];

    for height in test_heights {
        let supply_points = [0, 1, cap / 2, cap.saturating_sub(1), cap];

        for supply in supply_points {
            let reward = Economy::block_reward(height, supply);

            assert!(
                reward <= cap.saturating_sub(supply),
                "reward exceeded remaining mining allocation at height {} supply {}",
                height,
                supply
            );
        }
    }
}

#[test]
fn mainnet_chain_is_valid_immediately_after_creation() {
    let chain = Blockchain::new();

    assert!(chain.is_valid());
}

#[test]
fn mainnet_chain_state_is_deterministic_on_fresh_creation() {
    let first = Blockchain::new();
    let second = Blockchain::new();

    assert_eq!(first.len(), second.len());

    assert_eq!(first.genesis().hash(), second.genesis().hash());

    assert_eq!(first.mining_supply(), second.mining_supply());
}

#[test]
fn project_reserve_is_exactly_ten_percent_of_total_supply() {
    let genesis = GenesisState::new();

    assert_eq!(genesis.project_reserve(), genesis.total_supply() / 10);
}

#[test]
fn mining_allocation_is_ninety_percent_of_total_supply() {
    let genesis = GenesisState::new();

    assert_eq!(genesis.mining_allocation(), genesis.total_supply() * 9 / 10);
}
