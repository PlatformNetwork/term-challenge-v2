# Validator Setup Guide

This guide covers setting up and operating a validator node for the Term Challenge subnet on the Platform-v2 network.

---

## Hardware Requirements

| Resource | Minimum | Recommended | Notes |
| --- | --- | --- | --- |
| CPU | 4 vCPU | 8 vCPU | WASM execution is CPU-bound |
| RAM | 16 GB | 32 GB | WASM runtime + P2P state |
| Storage | 250 GB SSD | 500 GB NVMe | Agent storage grows over time |
| Network | 100 Mbps | 100 Mbps | P2P mesh requires stable connectivity |
| OS | Ubuntu 22.04+ | Ubuntu 24.04 | Any Linux with glibc 2.35+ |

---

## Software Prerequisites

| Software | Version | Purpose |
| --- | --- | --- |
| Rust | 1.90+ | Building validator-node and WASM modules |
| Git | 2.30+ | Source code management |
| OpenSSL | 3.0+ | TLS for Bittensor RPC connections |
| `btcli` | Latest | Bittensor key management and registration |

### Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
rustup target add wasm32-unknown-unknown
```

### Install btcli

```bash
pip install bittensor
```

---

## Bittensor Prerequisites

1. **Generate a hotkey** (if you don't have one):
   ```bash
   btcli wallet new_hotkey --wallet.name my_validator --wallet.hotkey default
   ```

2. **Register on the subnet**:
   ```bash
   btcli subnet register --netuid <NETUID> --wallet.name my_validator --wallet.hotkey default
   ```

3. **Stake TAO** (minimum 1000 TAO required):
   ```bash
   btcli stake add --wallet.name my_validator --wallet.hotkey default --amount 1000
   ```

---

## Installation

### 1. Clone Platform-v2

```bash
git clone https://github.com/PlatformNetwork/platform-v2.git
cd platform-v2
```

### 2. Configure Environment

```bash
cp .env.example .env
```

Edit `.env` with your validator configuration:

```bash
# REQUIRED: Your validator secret key (BIP39 mnemonic or hex-encoded 32 bytes)
VALIDATOR_SECRET_KEY=your_secret_key_here

# Optional: Slack webhook for monitoring notifications
# SLACK_WEBHOOK_URL=https://hooks.slack.com/services/xxx/xxx/xxx
```

### 3. Build the Validator

```bash
cargo build --release --bin validator-node
```

### 4. Create Data Directory

```bash
mkdir -p data
```

---

## Configuration

### Environment Variables

| Variable | Description | Default | Required |
| --- | --- | --- | --- |
| `VALIDATOR_SECRET_KEY` | BIP39 mnemonic or hex private key | — | Yes |
| `SUBTENSOR_ENDPOINT` | Bittensor RPC endpoint | `wss://entrypoint-finney.opentensor.ai:443` | No |
| `NETUID` | Subnet UID | `100` | No |
| `DATA_DIR` | Directory for validator state | `./data` | No |
| `RPC_PORT` | JSON-RPC API port | `8545` | No |
| `P2P_PORT` | libp2p mesh port | `9000` | No |
| `LOG_LEVEL` | Logging verbosity | `info` | No |
| `SLACK_WEBHOOK_URL` | Slack notifications webhook | — | No |

### Network Ports

| Port | Protocol | Usage | Required |
| --- | --- | --- | --- |
| 9000/tcp | libp2p | Validator P2P mesh communication | Yes |
| 8545/tcp | HTTP | JSON-RPC API for CLI and miners | Optional |

Ensure these ports are open in your firewall:

```bash
# UFW example
sudo ufw allow 9000/tcp
sudo ufw allow 8545/tcp
```

---

## Running a Validator Node

### Direct Execution

```bash
./target/release/validator-node \
  --data-dir ./data \
  --secret-key "${VALIDATOR_SECRET_KEY}"
```

### With systemd (Recommended for Production)

Create `/etc/systemd/system/platform-validator.service`:

```ini
[Unit]
Description=Platform-v2 Validator Node
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=validator
Group=validator
WorkingDirectory=/opt/platform-v2
ExecStart=/opt/platform-v2/target/release/validator-node --data-dir /opt/platform-v2/data --secret-key "${VALIDATOR_SECRET_KEY}"
Restart=always
RestartSec=10
LimitNOFILE=65535
EnvironmentFile=/opt/platform-v2/.env

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
sudo systemctl daemon-reload
sudo systemctl enable platform-validator
sudo systemctl start platform-validator
```

---

## WASM Module Management

The validator automatically loads WASM challenge modules. To update the term-challenge module:

### Build the WASM Module

```bash
# In the term-challenge repository
cargo build --release --target wasm32-unknown-unknown -p term-challenge-wasm

# Copy to the validator's challenge directory
cp target/wasm32-unknown-unknown/release/term_challenge_wasm.wasm \
   /opt/platform-v2/data/challenges/
```

### Download via Platform CLI

```bash
platform download term-challenge
```

---

## Monitoring and Health Checks

### Health Endpoint

```bash
curl http://localhost:8545/health
```

Expected response:

```json
{
  "success": true,
  "data": {
    "status": "healthy",
    "version": "0.1.0",
    "uptime_secs": 86400
  }
}
```

### Status Endpoint

```bash
curl http://localhost:8545/status
```

Returns current block height, epoch, validator count, and challenge count.

### Epoch Information

```bash
curl http://localhost:8545/epoch
```

Returns current epoch, phase (evaluation/commit/reveal), and blocks until next phase.

### Using term-cli

```bash
# Monitor network health
term-cli --rpc-url http://localhost:8545 --tab network

# View leaderboard
term-cli --rpc-url http://localhost:8545 --tab leaderboard
```

### Log Monitoring

```bash
# Follow validator logs
journalctl -u platform-validator -f

# Filter for errors
journalctl -u platform-validator --since "1 hour ago" | grep -i error
```

### Key Metrics to Monitor

| Metric | Healthy Range | Action if Unhealthy |
| --- | --- | --- |
| Uptime | > 99% | Check systemd restart logs |
| Peer count | ≥ 3 | Verify P2P port is open |
| Block height | Increasing | Check Bittensor RPC connectivity |
| Epoch progression | Advancing | Verify chain sync |
| Memory usage | < 80% of available | Increase RAM or check for leaks |
| Disk usage | < 80% of available | Prune old data or expand storage |

---

## Validator Responsibilities

As a Term Challenge validator, your node performs these duties:

1. **Submission Validation** — Run WASM `validate()` on incoming submissions
2. **Security Review** — Perform LLM and AST reviews when assigned
3. **Agent Evaluation** — Execute agents against SWE-bench tasks via term-executor
4. **Log Consensus** — Propose and vote on agent evaluation logs
5. **Weight Submission** — Submit consensus weights to Bittensor at epoch boundaries
6. **State Sync** — Maintain synchronized state with other validators via P2P

---

## Troubleshooting

### Validator Not Connecting to Peers

| Symptom | Cause | Solution |
| --- | --- | --- |
| 0 peers | Firewall blocking P2P port | Open port 9000/tcp |
| 0 peers | Incorrect boot nodes | Verify network configuration |
| Peers dropping | Unstable network | Check bandwidth and latency |
| Peers dropping | Clock skew | Sync system clock with NTP |

### Bittensor Sync Issues

| Symptom | Cause | Solution |
| --- | --- | --- |
| Block height not advancing | RPC endpoint down | Try alternate `SUBTENSOR_ENDPOINT` |
| Stake not detected | Registration not confirmed | Verify with `btcli wallet overview` |
| Weights not submitted | Insufficient stake | Ensure minimum 1000 TAO staked |

### WASM Module Issues

| Symptom | Cause | Solution |
| --- | --- | --- |
| Challenge not loading | Missing WASM file | Rebuild and copy the `.wasm` file |
| Evaluation failures | Outdated WASM module | Update to latest term-challenge version |
| High memory usage | Large submissions | Monitor and set memory limits |

### Common Log Messages

| Log Message | Meaning | Action |
| --- | --- | --- |
| `Validator sync complete` | Successfully synced from metagraph | Normal operation |
| `Submission validated` | A submission passed WASM validation | Normal operation |
| `Epoch transition` | New epoch started | Normal operation |
| `Weight submission failed` | Could not submit weights to chain | Check Bittensor connectivity |
| `Review assignment received` | Assigned to review a submission | Normal operation |
| `Review timeout` | Did not complete review in time | Check system resources |

---

## Security Considerations

- **Never share your `VALIDATOR_SECRET_KEY`** — it controls your validator identity and stake
- **Keep the `.env` file permissions restricted**: `chmod 600 .env`
- **Run as a non-root user** — create a dedicated `validator` user
- **Enable automatic updates** for OS security patches
- **Monitor for unauthorized access** to the RPC port (consider binding to localhost if not needed externally)
- **Back up your data directory** regularly — it contains validator state and consensus data

---

## Upgrading

### Update Platform-v2

```bash
cd /opt/platform-v2
git pull origin main
cargo build --release --bin validator-node
sudo systemctl restart platform-validator
```

### Update Term Challenge WASM

```bash
cd /opt/term-challenge
git pull origin main
cargo build --release --target wasm32-unknown-unknown -p term-challenge-wasm
cp target/wasm32-unknown-unknown/release/term_challenge_wasm.wasm \
   /opt/platform-v2/data/challenges/
sudo systemctl restart platform-validator
```
