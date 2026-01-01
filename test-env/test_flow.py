#!/usr/bin/env python3
"""
Full Flow Test with REAL sr25519 signatures
Tests: Miner submission -> Validator claim -> Evaluation -> Consensus
"""

import json
import time
import hashlib
import uuid
import requests
from substrateinterface import Keypair

SERVER = "http://localhost:8081"
API_KEY = "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"

# Load test keys
with open("/root/term-challenge-repo/test-env/test_keys.json") as f:
    KEYS = json.load(f)

# Simple test agent that solves hello-world task
AGENT_CODE = '''
from term_sdk import Agent, Request, Response, run

class HelloAgent(Agent):
    """Solves hello-world task - creates hello.txt with 'Hello, world!'"""
    
    def solve(self, req: Request) -> Response:
        # req.first is True on first step
        if req.first:
            return Response.cmd("echo 'Hello, world!' > hello.txt")
        return Response.done()

if __name__ == "__main__":
    run(HelloAgent())
'''

def get_keypair(name: str) -> Keypair:
    """Get keypair from stored keys"""
    return Keypair.create_from_mnemonic(KEYS[name]["seed"])

def sign_message(keypair: Keypair, message: str) -> str:
    """Sign message and return hex signature (64 bytes = 128 hex chars)"""
    sig = keypair.sign(message.encode())
    # substrate-interface returns bytes, convert to hex
    if isinstance(sig, bytes):
        return sig.hex()
    # If already hex string (0x prefix)
    if sig.startswith('0x'):
        return sig[2:]
    return sig

def log(msg: str):
    ts = time.strftime("%H:%M:%S")
    print(f"[{ts}] {msg}")

# ============================================================================
# STEP 1: MINER SUBMITS AGENT
# ============================================================================
def submit_agent():
    log("=" * 60)
    log("STEP 1: MINER SUBMITS AGENT")
    log("=" * 60)
    
    miner_kp = get_keypair("miner")
    log(f"Miner hotkey: {miner_kp.ss58_address}")
    
    # Create message to sign (same as server expects)
    code_hash = hashlib.sha256(AGENT_CODE.encode()).hexdigest()
    message = f"submit_agent:{code_hash}"
    log(f"Message to sign: {message}")
    
    signature = sign_message(miner_kp, message)
    log(f"Signature: {signature[:32]}...")
    
    payload = {
        "miner_hotkey": miner_kp.ss58_address,
        "source_code": AGENT_CODE,
        "language": "python",
        "name": "HelloAgent",
        "signature": signature,
        "api_key": API_KEY
    }
    
    r = requests.post(f"{SERVER}/api/v1/submit", json=payload, timeout=30)
    result = r.json()
    log(f"Response: {json.dumps(result, indent=2)}")
    
    if result.get("success"):
        log(f"SUCCESS! Agent hash: {result['agent_hash']}")
        return result["agent_hash"]
    else:
        log(f"FAILED: {result.get('error')}")
        return None

# ============================================================================
# STEP 2: VALIDATORS CLAIM JOBS
# ============================================================================
def validator_claim_jobs(validator_num: int, agent_hash: str):
    log(f"\n--- Validator {validator_num} claiming jobs ---")
    
    kp = get_keypair(f"validator_{validator_num}")
    timestamp = int(time.time())
    
    message = f"claim_jobs:{timestamp}"
    signature = sign_message(kp, message)
    
    payload = {
        "validator_hotkey": kp.ss58_address,
        "timestamp": timestamp,
        "signature": signature,
        "max_jobs": 10
    }
    
    r = requests.post(f"{SERVER}/api/v1/validator/claim_jobs", json=payload, timeout=30)
    result = r.json()
    
    if result.get("success"):
        jobs = result.get("jobs", [])
        log(f"Validator {validator_num} claimed {len(jobs)} jobs")
        for job in jobs:
            log(f"  - Agent: {job.get('agent_hash', 'N/A')[:16]}...")
        return jobs
    else:
        log(f"Validator {validator_num} claim failed: {result.get('error')}")
        return []

# ============================================================================
# STEP 3: VALIDATORS RUN REAL BENCHMARK AND SUBMIT RESULTS
# ============================================================================
def validator_run_benchmark(validator_num: int, job: dict):
    """Run real benchmark via /evaluate endpoint and submit result"""
    log(f"\n--- Validator {validator_num} running REAL benchmark ---")
    
    kp = get_keypair(f"validator_{validator_num}")
    agent_hash = job.get("agent_hash")
    source_code = job.get("source_code")
    miner_hotkey = job.get("miner_hotkey")
    submission_id = job.get("submission_id", str(uuid.uuid4()))
    
    if not source_code:
        log(f"ERROR: No source_code in job!")
        return None
    
    log(f"  Agent hash: {agent_hash[:16]}...")
    log(f"  Source code length: {len(source_code)} chars")
    
    # Step 1: Call /evaluate to run real benchmark
    log(f"  Calling /evaluate endpoint...")
    eval_payload = {
        "submission_id": submission_id,
        "agent_hash": agent_hash,
        "miner_hotkey": miner_hotkey,
        "validator_hotkey": kp.ss58_address,
        "source_code": source_code,
        "name": "test-agent",
        "epoch": 0,
    }
    
    try:
        eval_resp = requests.post(f"{SERVER}/evaluate", json=eval_payload, timeout=300)
        eval_result = eval_resp.json()
        
        if not eval_result.get("success"):
            log(f"  Evaluation failed: {eval_result.get('error')}")
            score = 0.0
            tasks_passed = 0
            tasks_failed = 1
            tasks_total = 1
        else:
            score = eval_result.get("score", 0.0)
            tasks_passed = eval_result.get("tasks_passed", 0)
            tasks_failed = eval_result.get("tasks_failed", 0)
            tasks_total = eval_result.get("tasks_total", 0)
            log(f"  BENCHMARK RESULT: score={score:.2%}, passed={tasks_passed}/{tasks_total}")
        
        execution_time_ms = eval_result.get("execution_time_ms", 0)
        total_cost_usd = eval_result.get("total_cost_usd", 0.0)
        task_results = eval_result.get("task_results", [])
        
    except Exception as e:
        log(f"  Evaluation request failed: {e}")
        score = 0.0
        tasks_passed = 0
        tasks_failed = 1
        tasks_total = 1
        execution_time_ms = 0
        total_cost_usd = 0.0
        task_results = []
    
    # Step 2: Submit result to server
    log(f"  Submitting result to server...")
    timestamp = int(time.time())
    message = f"submit_result:{agent_hash}:{timestamp}"
    signature = sign_message(kp, message)
    
    submit_payload = {
        "validator_hotkey": kp.ss58_address,
        "agent_hash": agent_hash,
        "score": score,
        "tasks_passed": tasks_passed,
        "tasks_failed": tasks_failed,
        "tasks_total": tasks_total,
        "execution_time_ms": execution_time_ms,
        "total_cost_usd": total_cost_usd,
        "task_results": task_results,
        "timestamp": timestamp,
        "signature": signature
    }
    
    r = requests.post(f"{SERVER}/api/v1/validator/submit_result", json=submit_payload, timeout=30)
    
    log(f"  Submit status: {r.status_code}")
    log(f"  Submit response: {r.text[:300] if r.text else 'empty'}")
    
    try:
        result = r.json()
    except:
        log(f"Failed to parse JSON response")
        return None
    
    if result.get("success"):
        log(f"Validator {validator_num} submitted: score={score}, consensus={result.get('consensus_reached')}")
        if result.get("final_score") is not None:
            log(f"CONSENSUS REACHED! Final score: {result['final_score']}")
        return result
    else:
        log(f"Validator {validator_num} submit failed: {result.get('error')}")
        return None

# ============================================================================
# STEP 4: CHECK LEADERBOARD
# ============================================================================
def check_leaderboard():
    log("\n" + "=" * 60)
    log("FINAL LEADERBOARD")
    log("=" * 60)
    
    r = requests.get(f"{SERVER}/leaderboard", timeout=10)
    result = r.json()
    
    entries = result.get("entries", [])
    if not entries:
        log("Leaderboard is empty")
        return
    
    for i, entry in enumerate(entries[:10]):
        name = entry.get("name") or "Unknown"
        score = entry.get("consensus_score", 0) or entry.get("best_score", 0)
        hotkey = entry.get("miner_hotkey", "")[:16]
        log(f"{i+1}. {name} - Score: {score:.2%} - Miner: {hotkey}...")

# ============================================================================
# MAIN
# ============================================================================
def main():
    log("=" * 60)
    log("TERM CHALLENGE - FULL FLOW TEST")
    log("=" * 60)
    
    # Check server
    try:
        r = requests.get(f"{SERVER}/health", timeout=5)
        if r.text != "OK":
            log("Server not ready")
            return
    except:
        log("Cannot connect to server")
        return
    
    log("Server is ready\n")
    
    # Step 1: Submit agent
    agent_hash = submit_agent()
    if not agent_hash:
        return
    
    time.sleep(2)
    
    # Step 2: All validators claim jobs
    log("\n" + "=" * 60)
    log("STEP 2: VALIDATORS CLAIM JOBS")
    log("=" * 60)
    
    claimed_jobs = {}  # validator_num -> list of jobs
    for i in range(1, 5):
        jobs = validator_claim_jobs(i, agent_hash)
        if jobs:
            claimed_jobs[i] = jobs
        time.sleep(0.5)
    
    time.sleep(2)
    
    # Step 3: Validators run REAL benchmark and submit results
    log("\n" + "=" * 60)
    log("STEP 3: VALIDATORS RUN REAL BENCHMARKS")
    log("=" * 60)
    
    for validator_num, jobs in claimed_jobs.items():
        for job in jobs:
            validator_run_benchmark(validator_num, job)
        time.sleep(1)
    
    time.sleep(2)
    
    # Step 4: Check leaderboard
    check_leaderboard()
    
    log("\n" + "=" * 60)
    log("TEST COMPLETE")
    log("=" * 60)

if __name__ == "__main__":
    main()
