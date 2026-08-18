use nio_blockchain::{Block, Blockchain, INITIAL_REWARD_UNITS};

fn mineable_block(chain: &Blockchain, height: u64, timestamp: u64, reward: u64) -> Block {
    let previous_hash = chain.latest_block().hash();

    Block::new(1, height, previous_hash, timestamp, 1, 0, reward)
}

#[test]
fn rejects_block_with_wrong_version() {
    let mut chain = Blockchain::new();

    let previous_hash = chain.latest_block().hash();

    let block = Block::new(999, 1, previous_hash, 60, 1, 0, INITIAL_REWARD_UNITS);

    let result = chain.add_block(block);

    assert!(result.is_err());
    assert_eq!(chain.len(), 1);
}

#[test]
fn rejects_block_with_wrong_height() {
    let mut chain = Blockchain::new();

    let previous_hash = chain.latest_block().hash();

    let block = Block::new(1, 99, previous_hash, 60, 1, 0, INITIAL_REWARD_UNITS);

    let result = chain.add_block(block);

    assert!(result.is_err());
    assert_eq!(chain.len(), 1);
}

#[test]
fn rejects_block_with_wrong_previous_hash() {
    let mut chain = Blockchain::new();

    let block = Block::new(1, 1, [9u8; 32], 60, 1, 0, INITIAL_REWARD_UNITS);

    let result = chain.add_block(block);

    assert!(result.is_err());
    assert_eq!(chain.len(), 1);
}

#[test]
fn rejects_timestamp_rollback() {
    let mut chain = Blockchain::new();

    let previous_timestamp = chain.latest_block().header.timestamp;

    let block = mineable_block(&chain, 1, previous_timestamp, INITIAL_REWARD_UNITS);

    let result = chain.add_block(block);

    assert!(result.is_err());
    assert_eq!(chain.len(), 1);
}

#[test]
fn rejects_excessive_mining_reward() {
    let mut chain = Blockchain::new();

    let previous_hash = chain.latest_block().hash();

    let block = Block::new(1, 1, previous_hash, 60, 1, 0, INITIAL_REWARD_UNITS + 1);

    let result = chain.add_block(block);

    assert!(result.is_err());
    assert_eq!(chain.len(), 1);
}

#[test]
fn rejects_zero_reward_before_mining_cap() {
    let mut chain = Blockchain::new();

    let previous_hash = chain.latest_block().hash();

    let block = Block::new(1, 1, previous_hash, 60, 1, 0, 0);

    let result = chain.add_block(block);

    assert!(result.is_err());
    assert_eq!(chain.len(), 1);
}

#[test]
fn rejected_block_does_not_change_chain() {
    let mut chain = Blockchain::new();

    let original_len = chain.len();
    let original_hash = chain.latest_block().hash();
    let original_supply = chain.mining_supply();

    let block = Block::new(1, 999, [1u8; 32], 1, 1, 0, u64::MAX);

    let result = chain.add_block(block);

    assert!(result.is_err());

    assert_eq!(chain.len(), original_len);
    assert_eq!(chain.latest_block().hash(), original_hash);
    assert_eq!(chain.mining_supply(), original_supply);
}

#[test]
fn blockchain_is_valid_after_rejected_attack() {
    let mut chain = Blockchain::new();

    let attack = Block::new(1, 1000, [0xffu8; 32], 999, 999, u64::MAX, u64::MAX);

    assert!(chain.add_block(attack).is_err());

    assert!(chain.is_valid());
}

#[test]
fn genesis_cannot_be_modified_into_extra_supply() {
    let mut chain = Blockchain::new();

    chain.blocks_mut()[0].miner_reward = u64::MAX;

    assert!(!chain.is_valid());
}

#[test]
fn genesis_hash_changes_when_genesis_is_tampered() {
    let mut chain = Blockchain::new();

    let original_hash = chain.genesis().hash();

    chain.blocks_mut()[0].header.timestamp = 9999;

    let modified_hash = chain.genesis().hash();

    assert_ne!(original_hash, modified_hash);
}

#[test]
fn tampering_with_latest_block_is_detectable() {
    let mut chain = Blockchain::new();

    let original_hash = chain.latest_block().hash();

    chain.blocks_mut()[0].header.nonce = 123456;

    let modified_hash = chain.latest_block().hash();

    assert_ne!(original_hash, modified_hash);
}
