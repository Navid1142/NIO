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
fn rejects_invalid_height() {
    let mut chain = Blockchain::new();

    let previous = chain.latest_block();

    let block = Block::new(
        1,
        previous.header.height + 99,
        previous.hash(),
        previous.header.timestamp + 60,
        1,
        0,
        INITIAL_REWARD_UNITS,
    );

    assert!(chain.add_block(block).is_err());
    assert_eq!(chain.len(), 1);
    assert!(chain.is_valid());
}

#[test]
fn rejects_invalid_previous_hash() {
    let mut chain = Blockchain::new();

    let previous = chain.latest_block();

    let block = Block::new(
        1,
        previous.header.height + 1,
        [0xAA; 32],
        previous.header.timestamp + 60,
        1,
        0,
        INITIAL_REWARD_UNITS,
    );

    assert!(chain.add_block(block).is_err());
    assert_eq!(chain.len(), 1);
    assert!(chain.is_valid());
}

#[test]
fn rejects_excessive_reward() {
    let mut chain = Blockchain::new();

    let block = make_block(&chain, INITIAL_REWARD_UNITS + 1);

    assert!(chain.add_block(block).is_err());
    assert_eq!(chain.len(), 1);
    assert_eq!(chain.mining_supply(), 0);
    assert!(chain.is_valid());
}

#[test]
fn detects_reward_tampering() {
    let mut chain = Blockchain::new();

    let block = make_block(&chain, INITIAL_REWARD_UNITS);

    chain
        .add_block(block)
        .expect("valid block should be accepted");

    assert!(chain.is_valid());

    chain.blocks_mut()[1].miner_reward += 1;

    assert!(!chain.is_valid());
}

#[test]
fn detects_height_tampering() {
    let mut chain = Blockchain::new();

    let block = make_block(&chain, INITIAL_REWARD_UNITS);

    chain
        .add_block(block)
        .expect("valid block should be accepted");

    assert!(chain.is_valid());

    chain.blocks_mut()[1].header.height += 10;

    assert!(!chain.is_valid());
}

#[test]
fn detects_previous_hash_tampering() {
    let mut chain = Blockchain::new();

    let block = make_block(&chain, INITIAL_REWARD_UNITS);

    chain
        .add_block(block)
        .expect("valid block should be accepted");

    assert!(chain.is_valid());

    chain.blocks_mut()[1].header.previous_hash = [0xBB; 32];

    assert!(!chain.is_valid());
}

#[test]
fn detects_timestamp_tampering() {
    let mut chain = Blockchain::new();

    let block = make_block(&chain, INITIAL_REWARD_UNITS);

    chain
        .add_block(block)
        .expect("valid block should be accepted");

    assert!(chain.is_valid());

    chain.blocks_mut()[1].header.timestamp += 1_000_000;

    assert!(!chain.is_valid());
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
