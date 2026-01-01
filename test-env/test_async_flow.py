#!/usr/bin/env python3
"""
Test Asynchronous Task Logging Flow

This script demonstrates the complete async workflow:
1. Miner submits agent
2. Validators claim jobs (receive source_code + task list)
3. Validators execute each task and log it in real-time
4. Validators submit final result (server verifies all logs present)
5. Consensus calculated when all validators complete

Usage:
    python test_async_flow.py
"""

import os
import sys
import json
import time
import uuid
import hashlib
import requests
from datetime import datetime

# Configuration
API_URL = os.getenv("API_URL", "http://localhost:8080")
MINER_HOTKEY = "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"  # Alice
VALIDATOR_HOTKEYS = [
    "5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty",  # Bob (V1)
    "5FLSigC9HGRKVhB9FiEo4Y3koPsNmBmLJbpXg2mp1hXcS59Y",  # Charlie (V2)
    "5DAAnrj7VHTznn2AWBemMuyBwZWs6FNFjdyVXUeYum3PTXFy",  # Dave (V3)
    "5HGjWAeFDfFCWPsjFQdVV2Msvz2XtMktvgocEZcCj68kUMaw",  # Eve (V4)
]

def sign_message(hotkey: str, message: str) -> str:
    """Generate a dummy signature (in production, use real crypto)"""
    return hashlib.sha256(f"{hotkey}:{message}".encode()).hexdigest()

def log_step(step: str, details: str = ""):
    """Log a test step"""
    timestamp = datetime.now().strftime("%H:%M:%S")
    print(f"\n[{timestamp}] === {step} ===")
    if details:
        print(f"    {details}")

def make_request(method: str, endpoint: str, data: dict = None, expected_success: bool = True):
    """Make HTTP request with error handling"""
    url = f"{API_URL}{endpoint}"
    try:
        if method == "GET":
            r = requests.get(url, timeout=30)
        else:
            r = requests.post(url, json=data, timeout=60)
        
        result = r.json() if r.content else {}
        
        if expected_success and r.status_code >= 400:
            print(f"    ERROR: {r.status_code} - {result}")
            return None
            
        return result
    except Exception as e:
        print(f"    Request failed: {e}")
        return None

def submit_agent() -> tuple[str, str]:
    """Step 1: Miner submits agent"""
    log_step("MINER: Submitting agent")
    
    source_code = '''
from term_sdk import run, Request, Response

def solve(req: Request) -> Response:
    """Simple test agent"""
    if req.first:
        return Response(command="echo 'Hello from async test!'")
    return Response(done=True)

if __name__ == "__main__":
    run(solve)
'''
    
    timestamp = int(time.time())
    agent_hash = hashlib.sha256(source_code.encode()).hexdigest()
    message = f"submit:{agent_hash}:{timestamp}"
    
    data = {
        "miner_hotkey": MINER_HOTKEY,
        "signature": sign_message(MINER_HOTKEY, message),
        "timestamp": timestamp,
        "source_code": source_code,
        "name": "AsyncTestAgent",
        "cost_limit_usd": 5.0,
    }
    
    result = make_request("POST", "/api/v1/submit", data)
    if result and result.get("success"):
        agent_hash = result["agent_hash"]
        submission_id = result["submission_id"]
        print(f"    Agent submitted: {agent_hash[:32]}...")
        print(f"    Submission ID: {submission_id}")
        return agent_hash, submission_id
    
    return None, None

def claim_jobs(validator_hotkey: str, validator_name: str) -> dict:
    """Step 2: Validator claims jobs"""
    log_step(f"VALIDATOR {validator_name}: Claiming jobs")
    
    timestamp = int(time.time())
    message = f"claim_jobs:{timestamp}"
    
    data = {
        "validator_hotkey": validator_hotkey,
        "signature": sign_message(validator_hotkey, message),
        "timestamp": timestamp,
        "max_jobs": 5,
    }
    
    result = make_request("POST", "/api/v1/validator/claim_jobs", data)
    if result:
        jobs = result.get("jobs", [])
        if jobs:
            job = jobs[0]
            print(f"    Claimed job: {job['agent_hash'][:32]}...")
            print(f"    Tasks to execute: {len(job.get('tasks', []))}")
            for task in job.get('tasks', [])[:3]:
                print(f"      - {task['task_id']}: {task['task_name']}")
            if len(job.get('tasks', [])) > 3:
                print(f"      ... and {len(job.get('tasks', [])) - 3} more")
            return job
        else:
            print("    No jobs available")
    return None

def log_task(validator_hotkey: str, validator_name: str, agent_hash: str, 
             task_id: str, task_name: str, passed: bool, score: float) -> dict:
    """Step 3: Log individual task result (real-time)"""
    timestamp = int(time.time())
    message = f"log_task:{agent_hash}:{task_id}:{timestamp}"
    
    data = {
        "validator_hotkey": validator_hotkey,
        "signature": sign_message(validator_hotkey, message),
        "timestamp": timestamp,
        "agent_hash": agent_hash,
        "task_id": task_id,
        "task_name": task_name,
        "passed": passed,
        "score": score,
        "execution_time_ms": 1500,  # Simulated execution time
        "steps": 3,
        "cost_usd": 0.01,
        "started_at": timestamp - 2,
    }
    
    result = make_request("POST", "/api/v1/validator/log_task", data)
    if result and result.get("success"):
        print(f"    [{validator_name}] Task {task_id} logged: {'PASS' if passed else 'FAIL'} "
              f"(score={score:.2f}) - Progress: {result['tasks_logged']}/{result['tasks_total']}")
        return result
    return None

def submit_final_result(validator_hotkey: str, validator_name: str, agent_hash: str,
                        score: float, tasks_passed: int, tasks_total: int,
                        skip_verification: bool = False) -> dict:
    """Step 4: Submit final evaluation result"""
    log_step(f"VALIDATOR {validator_name}: Submitting final result")
    
    timestamp = int(time.time())
    message = f"submit_result:{agent_hash}:{timestamp}"
    
    data = {
        "validator_hotkey": validator_hotkey,
        "signature": sign_message(validator_hotkey, message),
        "timestamp": timestamp,
        "agent_hash": agent_hash,
        "score": score,
        "tasks_passed": tasks_passed,
        "tasks_total": tasks_total,
        "tasks_failed": tasks_total - tasks_passed,
        "total_cost_usd": tasks_total * 0.01,
        "execution_time_ms": tasks_total * 1500,
        "skip_verification": skip_verification,
    }
    
    result = make_request("POST", "/api/v1/validator/submit_result", data)
    if result:
        if result.get("success"):
            print(f"    Result accepted!")
            print(f"    Validators: {result['validators_completed']}/{result['total_validators']}")
            if result.get("consensus_reached"):
                print(f"    CONSENSUS REACHED! Final score: {result['final_score']:.2%}")
        else:
            print(f"    Result rejected: {result.get('error')}")
    return result

def run_async_test():
    """Run complete async flow test"""
    print("=" * 70)
    print(" ASYNC TASK LOGGING FLOW TEST")
    print("=" * 70)
    
    # Step 1: Submit agent
    agent_hash, submission_id = submit_agent()
    if not agent_hash:
        print("\nFAILED: Could not submit agent")
        return False
    
    time.sleep(1)
    
    # Track which validators were assigned
    assigned_validators = []
    
    # Step 2: Validators claim jobs
    for i, (hotkey, name) in enumerate(zip(VALIDATOR_HOTKEYS[:4], ["V1", "V2", "V3", "V4"])):
        job = claim_jobs(hotkey, name)
        if job:
            assigned_validators.append((hotkey, name, job))
        time.sleep(0.5)
    
    if not assigned_validators:
        print("\nFAILED: No validators could claim jobs")
        return False
    
    print(f"\n    {len(assigned_validators)} validators assigned to this agent")
    
    # Step 3: Each validator executes tasks and logs them in real-time
    for hotkey, name, job in assigned_validators:
        log_step(f"VALIDATOR {name}: Executing tasks")
        
        tasks = job.get("tasks", [])
        if not tasks:
            print(f"    No tasks assigned, skipping...")
            continue
        
        # Execute and log each task
        tasks_passed = 0
        for task in tasks[:5]:  # Only test first 5 tasks for speed
            # Simulate task execution
            passed = True  # In real test, this would come from Docker evaluation
            score = 1.0 if passed else 0.0
            
            log_task(hotkey, name, agent_hash, task["task_id"], task["task_name"], passed, score)
            if passed:
                tasks_passed += 1
            
            time.sleep(0.2)  # Small delay between tasks
    
    # Step 4: Validators submit final results
    # Note: This should fail because not all 30 tasks are logged
    print("\n" + "=" * 50)
    print(" Testing verification: Submit with incomplete logs")
    print("=" * 50)
    
    for hotkey, name, job in assigned_validators[:1]:  # Test with first validator
        result = submit_final_result(
            hotkey, name, agent_hash, 
            score=0.8, tasks_passed=4, tasks_total=5,
            skip_verification=False
        )
        if result and not result.get("success"):
            print(f"    EXPECTED: Rejected because task logs incomplete")
    
    # Now test with skip_verification=True (backward compatibility)
    print("\n" + "=" * 50)
    print(" Testing backward compatibility: skip_verification=True")
    print("=" * 50)
    
    for hotkey, name, job in assigned_validators:
        result = submit_final_result(
            hotkey, name, agent_hash, 
            score=0.8, tasks_passed=4, tasks_total=5,
            skip_verification=True  # Skip verification for backward compatibility
        )
        time.sleep(0.3)
    
    # Check final status
    log_step("Checking leaderboard")
    result = make_request("GET", "/api/v1/leaderboard")
    if result and result.get("entries"):
        for entry in result["entries"][:3]:
            print(f"    {entry['miner_hotkey'][:16]}... - Score: {entry.get('consensus_score', 0):.2%}")
    
    print("\n" + "=" * 70)
    print(" TEST COMPLETE")
    print("=" * 70)
    return True

if __name__ == "__main__":
    success = run_async_test()
    sys.exit(0 if success else 1)
