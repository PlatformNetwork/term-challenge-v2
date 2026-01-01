#!/usr/bin/env python3
"""Generate 4 validator keypairs + 1 miner keypair for testing"""

from substrateinterface import Keypair
import json

# Generate 4 validators + 1 miner
keys = {}

print("=" * 60)
print("GENERATED TEST KEYPAIRS")
print("=" * 60)

# Validators
for i in range(1, 5):
    kp = Keypair.create_from_mnemonic(Keypair.generate_mnemonic())
    keys[f"validator_{i}"] = {
        "hotkey": kp.ss58_address,
        "seed": kp.mnemonic,
        "public_key": kp.public_key.hex()
    }
    print(f"\nValidator {i}:")
    print(f"  Hotkey: {kp.ss58_address}")
    print(f"  Mnemonic: {kp.mnemonic}")

# Miner
kp = Keypair.create_from_mnemonic(Keypair.generate_mnemonic())
keys["miner"] = {
    "hotkey": kp.ss58_address,
    "seed": kp.mnemonic,
    "public_key": kp.public_key.hex()
}
print(f"\nMiner:")
print(f"  Hotkey: {kp.ss58_address}")
print(f"  Mnemonic: {kp.mnemonic}")

# Save to file
with open("/root/term-challenge-repo/test-env/test_keys.json", "w") as f:
    json.dump(keys, f, indent=2)

print("\n" + "=" * 60)
print("Keys saved to test_keys.json")

# Print whitelist for server
validator_hotkeys = [keys[f"validator_{i}"]["hotkey"] for i in range(1, 5)]
print(f"\nVALIDATOR_WHITELIST={','.join(validator_hotkeys)}")
