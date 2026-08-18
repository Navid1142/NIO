use nio_blockchain::Blockchain;

fn main() {
    println!("========================================");
    println!("          NIO BLOCKCHAIN NODE");
    println!("========================================");

    let blockchain = Blockchain::new();

    println!("Network: NIO Mainnet");
    println!("Chain length: {}", blockchain.len());
    println!("Mining supply: {}", blockchain.mining_supply());

    let genesis = blockchain.genesis();
    let latest = blockchain.latest_block();

    println!("Genesis height: {}", genesis.header.height);
    println!("Latest height: {}", latest.header.height);
    println!("Blockchain valid: {}", blockchain.is_valid());

    println!("========================================");

    if blockchain.is_valid() {
        println!("NIO NODE STATUS: READY");
    } else {
        println!("NIO NODE STATUS: INVALID");
        std::process::exit(1);
    }
}
