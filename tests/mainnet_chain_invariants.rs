use nio_blockchain::{mining::Miner, Block, Blockchain, INITIAL_REWARD_UNITS};

fn mine_next_block(chain: &Blockchain, reward: u64) -> Block {
    let previous = chain.latest_block();

    let height = previous
        .header
        .height
        .checked_add(1)
        .expect("height must not overflow");

    let timestamp = previous
        .header
        .timestamp
        .checked_add(60)
        .expect("timestamp must not overflow");

    let difficulty = chain
        .expected_next_difficulty()
        .expect("difficulty calculation must succeed");

    let mut block = Block::new(1, height, previous.hash(), timestamp, difficulty, 0, reward);

    Miner::mine(&mut block).expect("test block must be mineable");

    block
}

#[test]
fn fresh_chain_has_canonical_initial_state() {
    let chain = Blockchain::new();

    assert!(!chain.is_empty());
    assert_eq!(chain.len(), 1);

    assert_eq!(chain.genesis().header.height, 0);
    assert_eq!(chain.latest_block().header.height, 0);

    assert_eq!(chain.mining_supply(), 0);

    assert!(chain.is_valid());
}

#[test]
fn fresh_chain_genesis_and_latest_are_same_block() {
    let chain = Blockchain::new();

    assert_eq!(chain.genesis().hash(), chain.latest_block().hash());
}

#[test]
fn fresh_chain_has_empty_utxo_state() {
    let chain = Blockchain::new();

    assert!(chain.utxo_set().is_empty());
    assert_eq!(chain.utxo_set().len(), 0);

    assert!(chain.is_valid());
}

#[test]
fn first_valid_block_preserves_chain_invariants() {
    let mut chain = Blockchain::new();

    let block = mine_next_block(&chain, INITIAL_REWARD_UNITS);

    assert_eq!(block.header.height, 1);
    assert_eq!(block.header.previous_hash, chain.latest_block().hash());

    chain
        .add_block(block)
        .expect("first valid block must be accepted");

    assert_eq!(chain.len(), 2);
    assert_eq!(chain.latest_block().header.height, 1);

    assert_eq!(chain.mining_supply(), INITIAL_REWARD_UNITS);

    assert!(chain.is_valid());
}

#[test]
fn second_valid_block_preserves_height_sequence() {
    let mut chain = Blockchain::new();

    let first = mine_next_block(&chain, INITIAL_REWARD_UNITS);
    chain
        .add_block(first)
        .expect("first block must be accepted");

    let second = mine_next_block(&chain, INITIAL_REWARD_UNITS);

    assert_eq!(second.header.height, 2);
    assert_eq!(second.header.previous_hash, chain.latest_block().hash());

    chain
        .add_block(second)
        .expect("second block must be accepted");

    assert_eq!(chain.len(), 3);
    assert_eq!(chain.latest_block().header.height, 2);

    assert!(chain.is_valid());
}

#[test]
fn invalid_height_does_not_modify_chain() {
    let mut chain = Blockchain::new();

    let original_len = chain.len();
    let original_hash = chain.latest_block().hash();
    let original_supply = chain.mining_supply();

    let previous = chain.latest_block();

    let difficulty = chain
        .expected_next_difficulty()
        .expect("difficulty calculation must succeed");

    let mut block = Block::new(
        1,
        2,
        previous.hash(),
        previous.header.timestamp + 60,
        difficulty,
        0,
        INITIAL_REWARD_UNITS,
    );

    Miner::mine(&mut block).expect("test block must be mineable");

    assert!(chain.add_block(block).is_err());

    assert_eq!(chain.len(), original_len);
    assert_eq!(chain.latest_block().hash(), original_hash);
    assert_eq!(chain.mining_supply(), original_supply);
    assert!(chain.is_valid());
}

#[test]
fn invalid_previous_hash_does_not_modify_chain() {
    let mut chain = Blockchain::new();

    let original_len = chain.len();
    let original_supply = chain.mining_supply();

    let difficulty = chain
        .expected_next_difficulty()
        .expect("difficulty calculation must succeed");

    let mut block = Block::new(1, 1, [0xffu8; 32], 60, difficulty, 0, INITIAL_REWARD_UNITS);

    Miner::mine(&mut block).expect("test block must be mineable");

    assert!(chain.add_block(block).is_err());

    assert_eq!(chain.len(), original_len);
    assert_eq!(chain.mining_supply(), original_supply);
    assert!(chain.is_valid());
}

#[test]
fn invalid_timestamp_does_not_modify_chain() {
    let mut chain = Blockchain::new();

    let original_len = chain.len();
    let original_supply = chain.mining_supply();

    let previous = chain.latest_block();

    let difficulty = chain
        .expected_next_difficulty()
        .expect("difficulty calculation must succeed");

    let mut block = Block::new(
        1,
        1,
        previous.hash(),
        previous.header.timestamp,
        difficulty,
        0,
        INITIAL_REWARD_UNITS,
    );

    Miner::mine(&mut block).expect("test block must be mineable");

    assert!(chain.add_block(block).is_err());

    assert_eq!(chain.len(), original_len);
    assert_eq!(chain.mining_supply(), original_supply);
    assert!(chain.is_valid());
}

#[test]
fn invalid_reward_does_not_modify_chain() {
    let mut chain = Blockchain::new();

    let original_len = chain.len();
    let original_supply = chain.mining_supply();

    let block = mine_next_block(&chain, INITIAL_REWARD_UNITS + 1);

    assert!(chain.add_block(block).is_err());

    assert_eq!(chain.len(), original_len);
    assert_eq!(chain.mining_supply(), original_supply);
    assert!(chain.is_valid());
}

#[test]
fn chain_remains_valid_after_multiple_blocks() {
    let mut chain = Blockchain::new();

    for _ in 0..10 {
        let reward = INITIAL_REWARD_UNITS;
        let block = mine_next_block(&chain, reward);

        chain
            .add_block(block)
            .expect("valid block must be accepted");

        assert!(chain.is_valid());
    }

    assert_eq!(chain.len(), 11);
    assert_eq!(chain.latest_block().header.height, 10);
}

#[test]
fn mining_supply_never_includes_genesis_reserve() {
    let chain = Blockchain::new();

    assert_eq!(chain.mining_supply(), 0);

    assert_eq!(
        nio_blockchain::genesis::GenesisState::new().project_reserve(),
        100 * nio_blockchain::economy::UNITS_PER_NIO
    );
}
