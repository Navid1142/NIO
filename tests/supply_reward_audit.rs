use nio_blockchain::economy::Economy;
use nio_blockchain::genesis::GenesisState;
use nio_blockchain::{mining::Miner, Block, Blockchain};

#[test]
fn canonical_supply_allocation_is_exact() {
    let genesis = GenesisState::new();

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
fn mining_starts_at_zero() {
    let chain = Blockchain::new();

    assert_eq!(chain.mining_supply(), 0);
    assert_eq!(GenesisState::new().initial_mining_supply(), 0);
}

#[test]
fn mining_cap_is_exactly_900_nio() {
    assert_eq!(
        Economy::mining_cap(),
        900 * nio_blockchain::economy::UNITS_PER_NIO
    );
}

#[test]
fn reserve_is_not_part_of_mining_cap() {
    let genesis = GenesisState::new();

    assert_eq!(
        genesis.mining_cap(),
        900 * nio_blockchain::economy::UNITS_PER_NIO
    );

    assert_eq!(
        genesis.project_reserve(),
        100 * nio_blockchain::economy::UNITS_PER_NIO
    );

    assert_ne!(genesis.mining_cap(), genesis.total_supply());
}

#[test]
fn supply_policy_is_valid() {
    assert!(Economy::policy_is_valid());
    assert!(GenesisState::new().is_valid());
}

#[test]
fn zero_supply_is_valid_initial_state() {
    assert!(Economy::is_supply_valid(0));
}

#[test]
fn mining_cap_is_valid_supply() {
    assert!(Economy::is_supply_valid(Economy::mining_cap()));
}

#[test]
fn supply_above_mining_cap_is_invalid() {
    assert!(!Economy::is_supply_valid(Economy::mining_cap() + 1));
}

#[test]
fn total_supply_is_not_a_mining_reward_cap() {
    let genesis = GenesisState::new();

    assert_eq!(
        genesis.total_supply(),
        genesis.project_reserve() + genesis.mining_allocation()
    );

    assert!(genesis.mining_cap() < genesis.total_supply());
}

#[test]
fn block_reward_never_exceeds_remaining_mining_supply() {
    let cap = Economy::mining_cap();
    let mut supply = 0u64;

    for height in 1..=1_000_000u64 {
        if supply >= cap {
            break;
        }

        let reward = Economy::block_reward(height, supply);

        assert!(
            reward <= cap - supply,
            "reward exceeded remaining mining allocation at height {}",
            height
        );

        supply = supply
            .checked_add(reward)
            .expect("supply addition must not overflow");

        assert!(supply <= cap);
    }

    assert!(supply <= cap);
}

#[test]
fn reward_at_or_above_mining_cap_is_zero() {
    let cap = Economy::mining_cap();

    assert_eq!(Economy::block_reward(1, cap), 0);

    assert_eq!(Economy::block_reward(1, cap + 1), 0);
}

#[test]
fn mining_supply_plus_reserve_never_exceeds_total_supply() {
    let genesis = GenesisState::new();

    let mut mining_supply = 0u64;

    for height in 1..=1_000_000u64 {
        if mining_supply >= genesis.mining_cap() {
            break;
        }

        let reward = Economy::block_reward(height, mining_supply);

        assert!(reward <= genesis.mining_cap() - mining_supply);

        mining_supply = mining_supply
            .checked_add(reward)
            .expect("mining supply must not overflow");

        let combined = mining_supply
            .checked_add(genesis.project_reserve())
            .expect("total supply calculation must not overflow");

        assert!(
            combined <= genesis.total_supply(),
            "total supply exceeded at height {}",
            height
        );
    }
}

#[test]
fn genesis_reserve_does_not_increase_mining_supply() {
    let chain = Blockchain::new();

    assert_eq!(chain.mining_supply(), 0);

    assert_eq!(
        GenesisState::new().project_reserve(),
        100 * nio_blockchain::economy::UNITS_PER_NIO
    );
}

#[test]
fn fees_are_not_counted_as_minted_supply() {
    let chain = Blockchain::new();

    assert_eq!(chain.mining_supply(), 0);

    // The chain's mining_supply() is based only on
    // Economy::block_reward(), not transaction fees.
    assert_eq!(chain.mining_supply(), 0);
}

#[test]
fn first_reward_is_bounded_by_mining_cap() {
    let reward = Economy::block_reward(1, 0);

    assert!(reward <= Economy::mining_cap());
}

#[test]
fn reward_calculation_handles_large_height_without_creating_extra_supply() {
    let cap = Economy::mining_cap();

    let heights = [1u64, 2, 210_240, 420_480, 630_720, 1_000_000, u64::MAX];

    for height in heights {
        let reward = Economy::block_reward(height, 0);

        assert!(
            reward <= cap,
            "reward exceeded mining cap at height {}",
            height
        );
    }
}

#[test]
fn chain_mining_supply_remains_bounded() {
    let mut chain = Blockchain::new();

    for height in 1..=100u64 {
        let previous = chain.latest_block();

        let supply = chain.mining_supply();
        let reward = Economy::block_reward(height, supply);

        assert!(supply <= Economy::mining_cap());

        assert!(reward <= Economy::mining_cap() - supply);

        let difficulty = chain
            .expected_next_difficulty()
            .expect("difficulty calculation must succeed");

        let mut block = Block::new(
            1,
            height,
            previous.hash(),
            previous.header.timestamp + 60,
            difficulty,
            0,
            reward,
        );

        Miner::mine(&mut block).expect("block must be mineable");

        chain
            .add_block(block)
            .expect("valid block must be accepted");

        assert!(chain.mining_supply() <= Economy::mining_cap());

        assert!(chain.is_valid());
    }
}
