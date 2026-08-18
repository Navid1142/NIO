use nio_blockchain::economy::Economy;
use nio_blockchain::genesis::GenesisState;

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

    assert_eq!(
        genesis.project_reserve() + genesis.mining_allocation(),
        genesis.total_supply()
    );

    assert_eq!(genesis.initial_mining_supply(), 0);

    assert_eq!(genesis.mining_cap(), Economy::mining_cap());

    assert!(genesis.reserve_is_outside_mining());
}

#[test]
fn mainnet_economy_matches_genesis() {
    let genesis = GenesisState::new();

    assert_eq!(Economy::total_supply(), genesis.total_supply());

    assert_eq!(Economy::project_reserve(), genesis.project_reserve());

    assert_eq!(Economy::mining_cap(), genesis.mining_allocation());

    assert!(Economy::policy_is_valid());
}

#[test]
fn mainnet_genesis_block_is_canonical() {
    let chain = nio_blockchain::Blockchain::new();

    let genesis = chain.genesis();

    assert_eq!(genesis.header.height, 0);
    assert_eq!(genesis.header.previous_hash, [0u8; 32]);
    assert_eq!(genesis.header.timestamp, 0);
    assert_eq!(genesis.miner_reward, 0);
    assert!(genesis.transactions.is_empty());
    assert_eq!(genesis.header.merkle_root, [0u8; 32]);

    assert!(chain.is_valid());
}

#[test]
fn mainnet_starts_with_zero_mining_supply() {
    let chain = nio_blockchain::Blockchain::new();

    assert_eq!(chain.mining_supply(), 0);
}

#[test]
fn mainnet_hard_cap_is_not_exceeded() {
    let genesis = GenesisState::new();

    assert!(genesis.mining_allocation() <= genesis.total_supply());

    assert_eq!(
        genesis.project_reserve() + genesis.mining_allocation(),
        genesis.total_supply()
    );
}
