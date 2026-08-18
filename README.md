# NIO Blockchain

NIO is an independent Proof-of-Work blockchain project built around a UTXO-based transaction model, cryptographic signatures, deterministic state validation, and a verification-first development process.
## Current Version

**NIO v0.1.0 — Initial Release**
Current initial node state:

- Network: NIO Mainnet
- Genesis Height: 0
- Initial Chain Length: 1
- Initial Mining Supply: 0
- Blockchain Validation: Enabled
- Node Status: READY

---
## Tokenomics

NIO has a fixed maximum supply.
| Allocation | Amount |
|---|---:|
| Total Hard Cap | 1,000 NIO |
| Mining Allocation | 900 NIO |
| Project Reserve | 100 NIO |
| Additional Supply | 0 NIO |
### Hard Cap

The maximum possible NIO supply is:
**1,000 NIO**

No additional NIO may ever be created beyond the hard cap.
### Project Reserve

The project reserve consists of:
**100 NIO**

The reserve is maintained separately from the mining allocation.
The reserve private key and keystore must remain outside the source code and must never be committed to the public repository.

---
## Core Architecture

NIO uses a UTXO-based blockchain architecture with Proof-of-Work consensus.
Major components include:

- Block structure and validation
- Blockchain state management
- Genesis validation
- Proof-of-Work mining
- Difficulty adjustment
- Mining economics
- Fixed supply validation
- Transaction processing
- Secp256k1 signatures
- UTXO state management
- Transaction fee validation
- Double-spend protection
- UTXO ownership validation
- Merkle-root validation
- Mempool
- P2P networking components
- Node runtime
- Wallet components
- Reserve and treasury components

---
## Security Model

Security is treated as a core development requirement.
The implementation includes validation against:

- Invalid block heights
- Invalid previous-block hashes
- Invalid timestamps
- Invalid difficulty
- Invalid Merkle roots
- Invalid Proof-of-Work
- Invalid transaction signatures
- Invalid transaction fees
- Unauthorized UTXO spending
- Duplicate transaction inputs
- Double spending
- UTXO state tampering
- Supply-cap violations
- Invalid state transitions
- Failed atomic transactions

---
## Cryptography

NIO currently uses:
- **SHA-256** for hashing
- **Secp256k1 / ECDSA** for transaction signatures

Cryptographic verification is performed during transaction validation.
---

## UTXO Model

Transactions consume existing unspent transaction outputs and create new outputs.
The UTXO layer provides:

- UTXO insertion
- UTXO spending
- Input-value calculation
- Output-value calculation
- Exact fee calculation
- Fee validation
- Ownership validation
- Signature validation
- Atomic transaction application
- Deterministic UTXO state hashing

The UTXO state hash provides a deterministic representation of the current UTXO set.
---

## Proof of Work

NIO uses Proof of Work for block production.
The blockchain validates:

- Block difficulty
- Proof-of-Work
- Block height
- Previous-block hash
- Block timestamp
- Mining reward
- Mining supply

Mining rewards are counted separately from transaction fees.
Transaction fees do not create new NIO supply.

---
## Genesis

The NIO blockchain starts from a deterministic genesis state.
Genesis validation ensures:

- Height is zero
- Previous hash is zero
- Genesis timestamp is valid
- Genesis reward is zero
- Genesis contains no normal transactions
- Initial mining supply is zero
- Mining cap matches the configured economy

---
## Node

The NIO node can be built in release mode and executed with Cargo.
Example:

```bash
cargo run --release --bin nio_node -- --help
```
