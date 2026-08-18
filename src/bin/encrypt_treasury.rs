use argon2::Argon2;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use getrandom::getrandom;
use rpassword::prompt_password;
use std::fs;
use std::path::Path;

fn main() {
    println!("==============================================");
    println!("NIO TREASURY KEY ENCRYPTOR");
    println!("==============================================");

    let dir = Path::new("treasury_secure");
    let private_path = dir.join("treasury_private_key.txt");
    let encrypted_path = dir.join("treasury_private_key.enc");

    if !private_path.exists() {
        panic!("treasury_private_key.txt was not found");
    }

    if encrypted_path.exists() {
        panic!("encrypted treasury key already exists");
    }

    let private_key_hex = fs::read_to_string(&private_path)
        .expect("failed to read treasury private key")
        .trim()
        .to_string();

    if private_key_hex.len() != 64 || !private_key_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        panic!("invalid treasury private key format");
    }

    let password = prompt_password("Enter Treasury password: ").expect("failed to read password");

    if password.is_empty() {
        panic!("password cannot be empty");
    }

    let confirmation = prompt_password("Confirm Treasury password: ")
        .expect("failed to read password confirmation");

    if password != confirmation {
        panic!("passwords do not match");
    }

    let mut salt = [0u8; 16];
    let mut nonce_bytes = [0u8; 12];

    getrandom(&mut salt).expect("failed to generate secure salt");

    getrandom(&mut nonce_bytes).expect("failed to generate secure nonce");

    let mut key_bytes = [0u8; 32];

    Argon2::default()
        .hash_password_into(password.as_bytes(), &salt, &mut key_bytes)
        .expect("failed to derive encryption key");

    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key_bytes));

    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), private_key_hex.as_bytes())
        .expect("failed to encrypt treasury private key");

    let encrypted_file = format!(
        "NIO-TREASURY-KEY-V1\nsalt={}\nnonce={}\nciphertext={}\n",
        STANDARD.encode(salt),
        STANDARD.encode(nonce_bytes),
        STANDARD.encode(ciphertext)
    );

    fs::write(&encrypted_path, encrypted_file).expect("failed to save encrypted treasury key");

    // Remove the plaintext private key only after
    // successful encryption.
    fs::remove_file(&private_path)
        .expect("encrypted successfully, but failed to remove plaintext key");

    // Verify the public information remains untouched.
    let address = fs::read_to_string(dir.join("treasury_address.txt"))
        .expect("failed to read treasury address");

    println!();
    println!("Treasury encryption completed successfully.");
    println!();
    println!("Treasury Address:");
    println!("{}", address.trim());

    println!();
    println!("Encrypted private key:");
    println!("treasury_secure\\treasury_private_key.enc");

    println!();
    println!("IMPORTANT:");
    println!("Your password is NOT stored in the project.");
    println!("DO NOT send your password or private key to anyone.");
    println!("Keep a secure backup of the password.");
    println!();
    println!("==============================================");
}
