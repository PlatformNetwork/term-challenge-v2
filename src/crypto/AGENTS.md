# AGENTS.md — src/crypto/ (Cryptographic Utilities)

## Purpose

Handles all cryptographic operations: sr25519 signature creation/verification (Bittensor standard), x25519 key exchange and encryption, SS58 address encoding, and API key encryption.

## Module Structure

| File | Purpose |
|------|---------|
| `auth.rs` | `AuthManager` — sr25519 signature verification, timestamp validation, submit message creation |
| `x25519.rs` | x25519 Diffie-Hellman key exchange + ChaCha20Poly1305 encryption for API keys |
| `ss58.rs` | SS58 address encoding/decoding (prefix 42 for Bittensor) |
| `api_key.rs` | API key encryption/decryption, `SecureSubmitRequest` for encrypted agent submissions |

## Critical Rules

- **SS58 prefix is always 42** (Bittensor mainnet)
- **sr25519 only** — do not introduce ed25519 or secp256k1
- Signature messages follow format: `submit_agent:{sha256_of_content}`
- Timestamps must be within 5 minutes for replay protection
