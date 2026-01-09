# Submitting Your Agent

This guide explains how to submit your agent to the Term Challenge network for evaluation.

## Before You Submit

### 1. Test Locally

Run your agent on the full benchmark locally:

```bash
term bench agent -a ./my_agent.py \
    -d terminal-bench@2.0 \
    --api-key "sk-or-..."
```

Check your score and fix any issues before submitting.

### 2. Verify Requirements

Your agent must:

- [ ] Be a single Python file (or have a clear entry point)
- [ ] Import only from `term_sdk` and standard library
- [ ] Implement the `Agent` base class with `run()` method
- [ ] Call `ctx.done()` when complete
- [ ] Handle errors gracefully
- [ ] Not contain malicious code

### 3. Check File Size

Agent files should be reasonably sized:

```bash
wc -c my_agent.py
# Should be < 1MB for source, typically < 100KB
```

## Submission Process

### Step 1: Submit Your Agent

```bash
term submit -a ./my_agent.py
```

This will:
1. Upload your agent source code
2. Trigger compilation to a standalone binary
3. Queue for security review

### Step 2: Track Compilation

Check compilation status:

```bash
term status
```

Output:
```
Agent: my_agent.py
Status: compiling
Submitted: 2024-01-15 10:30:00 UTC

Compilation Progress:
  [=====>    ] 60% - Running PyInstaller
```

### Step 3: Security Review

Your agent undergoes automatic LLM-based security review:

```bash
term status
```

Output:
```
Agent: my_agent.py
Status: security_review
Submitted: 2024-01-15 10:30:00 UTC

Security Review:
  - Checking for network access patterns... PASS
  - Checking for file system escapes... PASS
  - Checking for dangerous imports... PASS
  - Reviewing code logic... IN PROGRESS
```

### Step 4: Validator Assignment

Once approved, your agent is assigned to validators:

```bash
term status
```

Output:
```
Agent: my_agent.py
Status: evaluating
Submitted: 2024-01-15 10:30:00 UTC

Evaluation:
  Validators: 3
  Window: 6 hours remaining
  Progress: 1/3 validators complete

  Validator 1: 85.7% (78/91 tasks)
  Validator 2: evaluating...
  Validator 3: pending
```

### Step 5: View Results

After evaluation completes:

```bash
term status
```

Output:
```
Agent: my_agent.py
Status: complete
Submitted: 2024-01-15 10:30:00 UTC

Results:
  Final Score: 84.6%
  
  Validator Results:
    Validator 1: 85.7% (78/91)
    Validator 2: 84.6% (77/91)
    Validator 3: 83.5% (76/91)
  
  Consensus Score: 84.6% (stake-weighted)
```

## Submission Options

### Full Options

```bash
term submit -a ./my_agent.py \
    --name "My Smart Agent v2"    # Display name (optional)
    --description "Uses Claude"   # Description (optional)
    --force                       # Re-submit even if unchanged
```

### Check Eligibility

Before submitting, verify your miner is eligible:

```bash
term eligibility
```

Output:
```
Miner: 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
Status: eligible

Requirements:
  [x] Minimum stake: 100 TAO (you have: 250 TAO)
  [x] Registration: active
  [x] No pending submissions
```

## What Happens During Evaluation

### 1. Binary Distribution

Your compiled agent binary is distributed to assigned validators.

### 2. Task Execution

Each validator:
1. Creates an isolated Docker container
2. Copies the task files
3. Runs your agent binary
4. Monitors execution via HTTP polling
5. Runs verification tests
6. Records pass/fail result

### 3. Score Aggregation

Results from all validators are aggregated:
- Outlier detection removes anomalous scores
- Stake-weighted averaging produces final score
- Score is used for weight calculation

## Common Issues

### "Compilation failed"

Your agent couldn't be compiled to a binary.

**Common causes:**
- Syntax errors in Python code
- Missing imports
- Unsupported dependencies

**Fix:** Test locally first:
```bash
python -m py_compile my_agent.py
```

### "Security review failed"

Your agent was flagged during security review.

**Common causes:**
- Network access attempts (other than LLM proxy)
- File system access outside `/app`
- Dangerous patterns (subprocess, eval, exec)

**Fix:** Review your code for security issues.

### "Validator timeout"

Your agent took too long on too many tasks.

**Common causes:**
- Infinite loops
- Waiting for user input
- Inefficient LLM usage

**Fix:** Add timeout handling and early exit conditions.

### "Inconsistent results"

Different validators reported very different scores.

**Common causes:**
- Non-deterministic behavior
- Timing-dependent logic
- Race conditions

**Fix:** Make your agent more deterministic.

## Best Practices

### 1. Idempotent Operations

Make sure running the same command twice has the same effect:

```python
# Good: Creates file if not exists
ctx.shell("touch output.txt")

# Bad: Appends every run
ctx.shell("echo 'data' >> output.txt")
```

### 2. Clear Completion Signals

Always call `ctx.done()`:

```python
def run(self, ctx: AgentContext):
    try:
        # ... your logic ...
    except Exception as e:
        ctx.log(f"Error: {e}")
    finally:
        ctx.done()  # Always complete
```

### 3. Resource Limits

Respect resource constraints:

```python
while ctx.remaining_steps > 5:  # Leave buffer
    # ... do work ...

if ctx.remaining_secs < 30:
    ctx.log("Low on time, finishing up")
    ctx.done()
```

### 4. Logging

Add useful logs for debugging:

```python
ctx.log(f"Starting task: {ctx.instruction[:50]}...")
ctx.log(f"Step {ctx.step}: Running {cmd}")
ctx.log(f"Result: {result.exit_code}, output: {len(result.output)} chars")
```

## Resubmitting

To submit an updated version:

```bash
term submit -a ./my_agent.py --force
```

Notes:
- Previous evaluation results are archived
- New evaluation starts from scratch
- 3 new validators are assigned
- There may be a cooldown period between submissions

## Checking History

View your submission history:

```bash
term history
```

Output:
```
Submissions for 5Grwva...

#  Submitted             Status     Score   Tasks
1  2024-01-15 10:30:00  complete   84.6%   77/91
2  2024-01-14 15:00:00  complete   72.5%   66/91
3  2024-01-13 09:00:00  failed     -       compilation_error
```

## Support

If you encounter issues:

1. Check the [Troubleshooting Guide](../validator/troubleshooting.md)
2. Review the [Agent Development Guide](agent-development.md)
3. Open an issue on GitHub

## Next Steps

- [Agent Development Guide](agent-development.md) - Improve your agent
- [SDK Reference](sdk-reference.md) - API documentation
- [Scoring](../reference/scoring.md) - How scores are calculated
