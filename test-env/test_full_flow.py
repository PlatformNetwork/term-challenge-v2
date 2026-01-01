#!/usr/bin/env python3
"""
Full Flow Test - Simulates exact miner experience submitting an agent.

This script:
1. Generates a test miner keypair
2. Submits an agent to the central server
3. Monitors validator job claims
4. Watches consensus calculation
5. Checks final leaderboard
"""

import hashlib
import json
import time
import requests
import sys
from datetime import datetime

# Test configuration
CENTRAL_SERVER = "http://localhost:8081"
API_KEY = ""

# Simple test agent that solves hello-world task
TEST_AGENT_CODE = '''
from term_sdk import Agent, run_agent

class HelloAgent(Agent):
    """Simple agent that creates hello.txt"""
    
    async def step(self, instruction: str, screen: str, step: int) -> dict:
        # Parse instruction to understand what to do
        if "hello.txt" in instruction.lower() or "hello" in instruction.lower():
            return {
                "action": "command",
                "command": "echo 'Hello, world!' > hello.txt"
            }
        
        # If we see the file exists, we're done
        if "hello.txt" in screen or step > 2:
            return {"action": "task_complete", "message": "Created hello.txt"}
        
        # Default: try to create the file
        return {
            "action": "command",
            "command": "echo 'Hello, world!' > hello.txt"
        }

if __name__ == "__main__":
    run_agent(HelloAgent())
'''

def compute_agent_hash(source_code: str) -> str:
    """Compute SHA256 hash of agent source code"""
    return hashlib.sha256(source_code.encode()).hexdigest()

def create_signature(hotkey: str, message: str) -> str:
    """Create a mock signature for testing (in prod this uses sr25519)"""
    # For testing without actual crypto, we'll use a simple hash
    # Real implementation uses schnorrkel/sr25519
    return hashlib.sha256(f"{hotkey}:{message}".encode()).hexdigest()

def log(msg: str):
    """Log with timestamp"""
    print(f"[{datetime.now().strftime('%H:%M:%S')}] {msg}")

def check_health():
    """Check if central server is healthy"""
    try:
        r = requests.get(f"{CENTRAL_SERVER}/health", timeout=5)
        return r.status_code == 200
    except:
        return False

def check_detailed_health():
    """Get detailed health status"""
    try:
        r = requests.get(f"{CENTRAL_SERVER}/health/detailed", timeout=5)
        return r.json()
    except Exception as e:
        return {"error": str(e)}

def submit_agent(miner_hotkey: str, source_code: str, api_key: str = None):
    """Submit an agent like a real miner would"""
    message = f"submit:{hashlib.sha256(source_code.encode()).hexdigest()}"
    signature = create_signature(miner_hotkey, message)
    
    payload = {
        "miner_hotkey": miner_hotkey,
        "source_code": source_code,
        "language": "python",
        "name": "TestAgent",
        "signature": signature
    }
    
    if api_key:
        payload["api_key"] = api_key
    
    log(f"Submitting agent from miner {miner_hotkey[:16]}...")
    r = requests.post(f"{CENTRAL_SERVER}/api/v1/submit", json=payload, timeout=30)
    return r.json()

def get_leaderboard():
    """Get current leaderboard"""
    r = requests.get(f"{CENTRAL_SERVER}/leaderboard", timeout=10)
    return r.json()

def get_submission_status(agent_hash: str):
    """Get status of a submission"""
    r = requests.get(f"{CENTRAL_SERVER}/api/v1/status", timeout=10)
    return r.json()

def wait_for_server(max_wait=60):
    """Wait for central server to be ready"""
    log("Waiting for central server...")
    start = time.time()
    while time.time() - start < max_wait:
        if check_health():
            log("Central server is ready!")
            return True
        time.sleep(2)
    log("Timeout waiting for server")
    return False

def main():
    log("=" * 60)
    log("TERM CHALLENGE - FULL FLOW TEST")
    log("=" * 60)
    
    # Wait for server
    if not wait_for_server():
        sys.exit(1)
    
    # Check detailed health
    health = check_detailed_health()
    log(f"Health status: {json.dumps(health, indent=2)}")
    
    # Use a test miner hotkey (valid SS58 format)
    miner_hotkey = "5GNJqTPyNqANBkUVMN1LPPrxXnFouWXoe2wNSmmEoLctxiZY"
    
    # Submit our test agent
    log("\n--- STEP 1: Submit Agent ---")
    result = submit_agent(miner_hotkey, TEST_AGENT_CODE, API_KEY)
    log(f"Submission result: {json.dumps(result, indent=2)}")
    
    if not result.get("success"):
        log(f"Submission failed: {result.get('error')}")
        sys.exit(1)
    
    agent_hash = result.get("agent_hash")
    log(f"Agent hash: {agent_hash}")
    
    # Monitor progress
    log("\n--- STEP 2: Monitor Evaluation ---")
    for i in range(30):  # Wait up to 5 minutes
        time.sleep(10)
        
        # Check leaderboard
        leaderboard = get_leaderboard()
        log(f"Leaderboard entries: {len(leaderboard.get('entries', []))}")
        
        # Look for our agent
        for entry in leaderboard.get("entries", []):
            if entry.get("agent_hash") == agent_hash:
                log(f"Agent found! Score: {entry.get('best_score')}")
                log(f"Full entry: {json.dumps(entry, indent=2)}")
                break
        
        # Get status
        status = get_submission_status(agent_hash)
        log(f"Status: {json.dumps(status, indent=2)}")
        
        if status.get("status") == "completed":
            log("\n=== EVALUATION COMPLETE ===")
            break
    
    # Final leaderboard
    log("\n--- FINAL LEADERBOARD ---")
    leaderboard = get_leaderboard()
    for i, entry in enumerate(leaderboard.get("entries", [])[:10]):
        log(f"{i+1}. {entry.get('name', 'Unknown')} - Score: {entry.get('best_score', 0):.2%}")
    
    log("\n" + "=" * 60)
    log("TEST COMPLETE")
    log("=" * 60)

if __name__ == "__main__":
    main()
