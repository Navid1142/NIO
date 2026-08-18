use nio_blockchain::wallet::Wallet;
use std::fs;
use std::path::Path;

fn main() {
    println!("==============================================");
    println!("NIO TREASURY KEY GENERATOR");
    println!("==============================================");

    let wallet = Wallet::new().expect("failed to generate treasury wallet");

    let address = wallet.address();
    let public_key = wallet.public_key_bytes();

    // Private key is stored locally only.
    // NEVER print it to the terminal or send it anywhere.
    let private_key_hex = wallet
        .secret_key()
        .secret_bytes()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();

    let treasury_dir = Path::new("treasury_secure");

    fs::create_dir_all(treasury_dir).expect("failed to create treasury_secure directory");

    let private_key_path = treasury_dir.join("treasury_private_key.txt");
    let public_key_path = treasury_dir.join("treasury_public_key.txt");
    let address_path = treasury_dir.join("treasury_address.txt");

    if private_key_path.exists() || public_key_path.exists() || address_path.exists() {
        panic!("Treasury files already exist. Refusing to overwrite existing keys.");
    }

    fs::write(&private_key_path, format!("{}\n", private_key_hex))
        .expect("failed to save private key");

    fs::write(&public_key_path, format!("{:02x?}\n", public_key))
        .expect("failed to save public key");

    fs::write(&address_path, format!("{}\n", address)).expect("failed to save treasury address");

    println!();
    println!("Treasury Address:");
    println!("{}", address);

    println!();
    println!("Treasury Public Key:");
    println!("{:02x?}", public_key);

    println!();
    println!("Treasury wallet created successfully.");
    println!();
    println!("Private key saved locally to:");
    println!("treasury_secure\\treasury_private_key.txt");

    println!();
    println!("IMPORTANT:");
    println!("DO NOT send the private key to ChatGPT.");
    println!("DO NOT publish the private key.");
    println!("Keep a secure backup of the private key.");
    println!();
    println!("==============================================");
}
