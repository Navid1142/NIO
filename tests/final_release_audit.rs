use nio_blockchain::economy::Economy;
use nio_blockchain::genesis::GenesisState;
use nio_blockchain::Blockchain;

#[test]
fn final_genesis_invariant() {
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

    assert_eq!(
        genesis.project_reserve() + genesis.mining_allocation(),
        genesis.total_supply()
    );
}

#[test]
fn final_supply_invariant() {
    let genesis = GenesisState::new();

    assert!(Economy::policy_is_valid());

    assert_eq!(Economy::total_supply(), genesis.total_supply());

    assert_eq!(Economy::project_reserve(), genesis.project_reserve());

    assert_eq!(Economy::mining_cap(), genesis.mining_allocation());

    assert!(Economy::is_supply_valid(0));
    assert!(Economy::is_supply_valid(Economy::mining_cap()));

    assert!(!Economy::is_supply_valid(Economy::mining_cap() + 1));
}

#[test]
fn final_reward_invariant() {
    let cap = Economy::mining_cap();

    let heights = [
        1u64,
        2,
        10,
        210_240,
        210_241,
        420_480,
        840_960,
        1_000_000,
        u64::MAX,
    ];

    for height in heights {
        let reward = Economy::block_reward(height, 0);

        assert!(
            reward <= cap,
            "reward exceeds mining cap at height {}",
            height
        );

        assert_eq!(Economy::block_reward(height, cap), 0);
    }
}

#[test]
fn final_fresh_chain_invariant() {
    let chain = Blockchain::new();

    assert_eq!(chain.len(), 1);
    assert!(!chain.is_empty());

    assert_eq!(chain.genesis().header.height, 0);

    assert_eq!(chain.genesis().header.previous_hash, [0u8; 32]);

    assert_eq!(chain.genesis().header.timestamp, 0);

    assert_eq!(chain.genesis().miner_reward, 0);

    assert!(chain.genesis().transactions.is_empty());

    assert_eq!(chain.mining_supply(), 0);

    assert!(chain.is_valid());
}

#[test]
fn final_genesis_is_deterministic() {
    let first = Blockchain::new();
    let second = Blockchain::new();

    assert_eq!(first.genesis().hash(), second.genesis().hash());
}

#[test]
fn final_chain_validation_is_stable() {
    let chain = Blockchain::new();

    for _ in 0..100 {
        assert!(chain.is_valid());
    }
}

#[test]
fn final_supply_cannot_include_project_reserve() {
    let genesis = GenesisState::new();

    assert_eq!(genesis.initial_mining_supply(), 0);

    assert_eq!(
        genesis.mining_cap(),
        900 * nio_blockchain::economy::UNITS_PER_NIO
    );

    assert_eq!(
        genesis.project_reserve(),
        100 * nio_blockchain::economy::UNITS_PER_NIO
    );

    assert!(genesis.mining_cap() < genesis.total_supply());
}

#[test]
fn final_supply_combination_is_exact() {
    let genesis = GenesisState::new();

    let combined = genesis
        .project_reserve()
        .checked_add(genesis.mining_allocation())
        .expect("allocation addition must not overflow");

    assert_eq!(combined, genesis.total_supply());
}

#[test]
fn final_reward_never_exceeds_remaining_cap() {
    let cap = Economy::mining_cap();

    let supplies = [0u64, 1, cap / 4, cap / 2, cap.saturating_sub(1), cap];

    let heights = [1u64, 2, 210_240, 420_480, 840_960, 1_000_000, u64::MAX];

    for height in heights {
        for supply in supplies {
            let reward = Economy::block_reward(height, supply);

            let remaining = cap.saturating_sub(supply);

            assert!(
                reward <= remaining,
                "reward exceeded remaining cap: height={}, supply={}",
                height,
                supply
            );
        }
    }
}

#[test]
fn final_policy_remains_valid_after_repeated_reads() {
    for _ in 0..1_000 {
        assert!(Economy::policy_is_valid());
        assert!(GenesisState::new().is_valid());
    }
}

#[test]
fn final_chain_and_economy_agree() {
    let chain = Blockchain::new();
    let genesis = GenesisState::new();

    assert_eq!(chain.mining_supply(), genesis.initial_mining_supply());

    assert_eq!(genesis.mining_cap(), Economy::mining_cap());

    assert_eq!(genesis.total_supply(), Economy::total_supply());
}
