use nio_blockchain::{mining::Miner, Block, Blockchain, INITIAL_REWARD_UNITS};

fn make_block(chain: &Blockchain, reward: u64) -> Block {
    let previous = chain.latest_block();

    let mut block = Block::new(
        1,
        previous.header.height + 1,
        previous.hash(),
        previous.header.timestamp + 60,
        1,
        0,
        reward,
    );

    Miner::mine(&mut block).expect("valid block should be mineable");

    block
}

#[test]
fn mining_supply_increases_after_valid_block() {
    let mut chain = Blockchain::new();

    assert_eq!(chain.mining_supply(), 0);

    let block = make_block(&chain, INITIAL_REWARD_UNITS);

    chain
        .add_block(block)
        .expect("valid block should be accepted");

    assert_eq!(chain.mining_supply(), INITIAL_REWARD_UNITS);
}

#[test]
fn rejected_block_does_not_modify_chain() {
    let mut chain = Blockchain::new();

    let original_length = chain.len();
    let original_supply = chain.mining_supply();

    let block = make_block(&chain, INITIAL_REWARD_UNITS + 1);

    assert!(chain.add_block(block).is_err());

    assert_eq!(chain.len(), original_length);

    assert_eq!(chain.mining_supply(), original_supply);

    assert!(chain.is_valid());
}

#[test]
fn tampered_block_hash_changes() {
    let mut chain = Blockchain::new();

    let block = make_block(&chain, INITIAL_REWARD_UNITS);

    chain
        .add_block(block)
        .expect("valid block should be accepted");

    let original = chain.latest_block().clone();

    let mut tampered = original.clone();

    tampered.miner_reward += 1;

    assert_ne!(original.hash(), tampered.hash());
}

#[test]
fn genesis_and_latest_block_heights_are_correct() {
    let mut chain = Blockchain::new();

    assert_eq!(chain.genesis().header.height, 0);

    let block = make_block(&chain, INITIAL_REWARD_UNITS);

    chain
        .add_block(block)
        .expect("valid block should be accepted");

    assert_eq!(chain.latest_block().header.height, 1);
}
