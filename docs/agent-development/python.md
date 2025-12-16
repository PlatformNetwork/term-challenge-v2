# Python Agent Development

Complete guide for building Term Challenge agents in Python.

## Installation

Install from the git repository:

```bash
# Clone the repository
git clone https://github.com/PlatformNetwork/term-challenge.git
cd term-challenge

# Install the SDK
pip install -e sdk/python
```

Or install directly from git:

```bash
pip install git+https://github.com/PlatformNetwork/term-challenge.git#subdirectory=sdk/python
```

## SDK Overview

```python
from term_sdk import (
    # Core harness types
    Agent,           # Base class for agents
    AgentRequest,    # Request from harness
    AgentResponse,   # Response to harness
    Command,         # Terminal command
    Harness,         # Agent runner
    run,             # Convenience function
    
    # LLM client
    LLMClient,       # Multi-provider LLM client
    Provider,        # Provider enum
    Message,         # Chat message
    ChatResponse,    # LLM response
    CostTracker,     # Token/cost tracking
)
```

## Basic Agent Structure

```python
#!/usr/bin/env python3
from term_sdk import Agent, AgentResponse, Command, run

class MyAgent(Agent):
    """Your custom agent implementation."""
    
    async def setup(self) -> None:
        """Initialize resources (optional)."""
        pass
    
    async def step(self, instruction: str, screen: str, step: int) -> AgentResponse:
        """
        Process one step of the task.
        
        Args:
            instruction: The task goal/description
            screen: Current terminal content
            step: Step number (1-indexed)
        
        Returns:
            AgentResponse with analysis, plan, commands, and completion status
        """
        # Your logic here
        return AgentResponse(
            analysis="What I observe...",
            plan="What I'll do...",
            commands=[Command("ls -la\n", duration=0.5)],
            task_complete=False
        )
    
    async def cleanup(self) -> None:
        """Clean up resources (optional)."""
        pass

if __name__ == "__main__":
    run(MyAgent())
```

## Core Types

### Command

```python
from term_sdk import Command

# Basic command with Enter
cmd = Command("ls -la\n")

# Command with custom duration
cmd = Command("pip install numpy\n", duration=10.0)

# Special keys (tmux-style)
cmd = Command("C-c")  # Ctrl+C
cmd = Command("Tab")  # Tab
cmd = Command("Escape")  # Escape
```

### AgentResponse

```python
from term_sdk import AgentResponse, Command

response = AgentResponse(
    analysis="Terminal shows an empty directory",
    plan="Create the requested file using echo",
    commands=[
        Command("echo 'Hello' > hello.txt\n", duration=0.3),
        Command("cat hello.txt\n", duration=0.3)
    ],
    task_complete=False
)

# Create error response
error_response = AgentResponse.error("Something went wrong")
```

### AgentRequest

```python
from term_sdk import AgentRequest

# Parse from dict (automatically done by Harness)
request = AgentRequest.from_dict({
    "instruction": "Create a file",
    "screen": "$ ",
    "step": 1
})

print(request.instruction)  # "Create a file"
print(request.screen)       # "$ "
print(request.step)         # 1
```

## LLM Integration

### Basic LLM Agent

```python
from term_sdk import Agent, AgentResponse, Command, run, LLMClient

class LLMAgent(Agent):
    async def setup(self):
        # Create client (uses OPENROUTER_API_KEY env var)
        self.client = LLMClient(provider="openrouter")
    
    async def step(self, instruction: str, screen: str, step: int) -> AgentResponse:
        # Build prompt
        prompt = f"""Task: {instruction}

Terminal (step {step}):
```
{screen}
```

Respond with JSON:
{{
  "analysis": "your analysis",
  "plan": "your plan",
  "commands": [{{"keystrokes": "...", "duration": 1.0}}],
  "task_complete": false
}}"""
        
        # Call LLM
        response = await self.client.chat([
            {"role": "system", "content": "You are a terminal expert."},
            {"role": "user", "content": prompt}
        ])
        
        # Parse response
        return self.parse_response(response.content)
    
    def parse_response(self, content: str) -> AgentResponse:
        import json
        
        # Find JSON in response
        start = content.find('{')
        end = content.rfind('}') + 1
        
        if start < 0 or end <= start:
            return AgentResponse.error("No JSON in response")
        
        try:
            data = json.loads(content[start:end])
            return AgentResponse(
                analysis=data.get("analysis", ""),
                plan=data.get("plan", ""),
                commands=[
                    Command(c["keystrokes"], c.get("duration", 1.0))
                    for c in data.get("commands", [])
                ],
                task_complete=data.get("task_complete", False)
            )
        except json.JSONDecodeError as e:
            return AgentResponse.error(f"Invalid JSON: {e}")

if __name__ == "__main__":
    run(LLMAgent())
```

### LLMClient Configuration

```python
from term_sdk import LLMClient

# OpenRouter (default)
client = LLMClient(
    provider="openrouter",
    model="anthropic/claude-3-haiku",  # Optional
    api_key="sk-or-...",               # Or use OPENROUTER_API_KEY env
    budget=5.0,                        # Max $5 per session
    timeout=300                        # 5 minute timeout
)

# Chutes
client = LLMClient(
    provider="chutes",
    model="Qwen/Qwen3-32B"
)

# OpenAI
client = LLMClient(
    provider="openai",
    model="gpt-4o-mini"
)

# Anthropic (direct)
client = LLMClient(
    provider="anthropic",
    model="claude-3-haiku-20240307"
)
```

### Chat Options

```python
response = await client.chat(
    messages=[
        {"role": "system", "content": "You are helpful."},
        {"role": "user", "content": "Hello!"}
    ],
    model="gpt-4o",           # Override model
    temperature=0.7,          # Sampling temperature
    max_tokens=4096           # Max response tokens
)

# Response fields
print(response.content)          # Text response
print(response.prompt_tokens)    # Input tokens
print(response.completion_tokens) # Output tokens
print(response.cost)             # Cost in USD
print(response.latency_ms)       # Response time
```

### Cost Tracking

```python
from term_sdk import estimate_cost

# Estimate before calling
cost = estimate_cost(
    model="anthropic/claude-3-haiku",
    prompt_tokens=1000,
    completion_tokens=500
)
print(f"Estimated: ${cost:.4f}")

# Track actual usage
client = LLMClient(budget=10.0)
# ... make calls ...
print(f"Total cost: ${client.total_cost:.4f}")
print(f"Total tokens: {client.total_tokens}")
print(f"Requests: {client.request_count}")
```

## Advanced Patterns

### Conversation History

```python
class ConversationalAgent(Agent):
    async def setup(self):
        self.client = LLMClient()
        self.history = []
    
    async def step(self, instruction: str, screen: str, step: int) -> AgentResponse:
        # Add current state to history
        self.history.append({
            "role": "user",
            "content": f"Step {step}:\n{screen}"
        })
        
        # Keep history manageable
        if len(self.history) > 20:
            self.history = self.history[-20:]
        
        # Build messages
        messages = [
            {"role": "system", "content": f"Task: {instruction}"},
            *self.history
        ]
        
        response = await self.client.chat(messages)
        
        # Add response to history
        self.history.append({
            "role": "assistant",
            "content": response.content
        })
        
        return self.parse_response(response.content)
```

### Error Recovery

```python
class RobustAgent(Agent):
    async def step(self, instruction: str, screen: str, step: int) -> AgentResponse:
        # Detect common errors
        if "command not found" in screen:
            return AgentResponse(
                analysis="Previous command not found",
                plan="Try alternative command",
                commands=[Command("which python3\n", 0.3)]
            )
        
        if "Permission denied" in screen:
            return AgentResponse(
                analysis="Permission error detected",
                plan="Try with sudo or different approach",
                commands=[Command("sudo !!\n", 1.0)]
            )
        
        # Normal processing...
        return await self.normal_step(instruction, screen, step)
```

### Progress Tracking

```python
class ProgressAgent(Agent):
    async def setup(self):
        self.steps_taken = 0
        self.max_steps = 50
    
    async def step(self, instruction: str, screen: str, step: int) -> AgentResponse:
        self.steps_taken += 1
        
        # Force completion if too many steps
        if self.steps_taken >= self.max_steps:
            return AgentResponse(
                analysis="Max steps reached",
                plan="Marking task complete",
                commands=[],
                task_complete=True
            )
        
        # Normal processing...
```

## Logging

Use stderr for logging (stdout is reserved for protocol):

```python
import sys
import logging

# Configure logging to stderr
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s [%(levelname)s] %(message)s',
    stream=sys.stderr
)
logger = logging.getLogger(__name__)

class MyAgent(Agent):
    async def step(self, instruction: str, screen: str, step: int) -> AgentResponse:
        logger.info(f"Processing step {step}")
        logger.debug(f"Screen: {screen[:100]}...")
        
        # ...
```

## Testing Your Agent

### Local Testing

```bash
# Test with a single task
term bench agent -a ./my_agent.py -t /path/to/task \
    --provider openrouter \
    --model anthropic/claude-3-haiku

# Run full benchmark
term bench benchmark terminal-bench@2.0 -a ./my_agent.py

# With budget limit
term bench agent -a ./my_agent.py -t /path/to/task --budget 1.0
```

### Unit Testing

```python
import pytest
from my_agent import MyAgent

@pytest.mark.asyncio
async def test_agent_step():
    agent = MyAgent()
    await agent.setup()
    
    response = await agent.step(
        instruction="List files",
        screen="$ ",
        step=1
    )
    
    assert response.analysis
    assert response.commands
    assert not response.task_complete

@pytest.mark.asyncio
async def test_completion_detection():
    agent = MyAgent()
    await agent.setup()
    
    response = await agent.step(
        instruction="Create test.txt",
        screen="$ cat test.txt\nHello World\n$ ",
        step=5
    )
    
    # Agent should recognize task is complete
    assert response.task_complete
```

## Validation & Submission

### Validate

```bash
term validate --file my_agent.py
```

Checks:
- Module whitelist compliance
- No forbidden builtins (exec, eval, etc.)
- Valid structure

### Submit

```bash
# Submit to Platform
term upload --file my_agent.py -k YOUR_HOTKEY

# Check status
term status --hash SUBMISSION_HASH
```

## Complete Example

```python
#!/usr/bin/env python3
"""
Complete LLM-powered terminal agent for Term Challenge.
"""
import json
import sys
import logging
from typing import Optional

from term_sdk import Agent, AgentResponse, Command, run, LLMClient

# Logging to stderr
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s [%(levelname)s] %(message)s',
    stream=sys.stderr
)
logger = logging.getLogger(__name__)

SYSTEM_PROMPT = """You are an expert terminal agent. Complete tasks using shell commands.

Rules:
1. Analyze the terminal output carefully
2. Execute one logical step at a time
3. Verify your actions worked before proceeding
4. Use appropriate wait durations
5. Set task_complete=true only when verified complete

Respond with JSON:
{
  "analysis": "What you observe in the terminal",
  "plan": "What you will do next",
  "commands": [{"keystrokes": "command\\n", "duration": 1.0}],
  "task_complete": false
}
"""

class TerminalAgent(Agent):
    def __init__(self, model: str = "anthropic/claude-3-haiku"):
        self.model = model
        self.client: Optional[LLMClient] = None
        self.conversation = []
    
    async def setup(self):
        logger.info(f"Initializing agent with model: {self.model}")
        self.client = LLMClient(
            provider="openrouter",
            model=self.model,
            budget=10.0
        )
    
    async def step(self, instruction: str, screen: str, step: int) -> AgentResponse:
        logger.info(f"Step {step}: Processing")
        
        # Build user message
        user_msg = f"""Task: {instruction}

Current Terminal (Step {step}):
```
{screen[-2000:]}
```

What's your next action?"""
        
        # Update conversation
        self.conversation.append({"role": "user", "content": user_msg})
        
        # Keep conversation manageable
        if len(self.conversation) > 10:
            self.conversation = self.conversation[-10:]
        
        # Call LLM
        try:
            response = await self.client.chat(
                messages=[
                    {"role": "system", "content": SYSTEM_PROMPT},
                    *self.conversation
                ],
                temperature=0.3,
                max_tokens=2048
            )
            
            logger.info(f"LLM response ({response.latency_ms}ms, ${response.cost:.4f})")
            
            # Add to conversation
            self.conversation.append({
                "role": "assistant", 
                "content": response.content
            })
            
            return self.parse_response(response.content)
            
        except Exception as e:
            logger.error(f"LLM error: {e}")
            return AgentResponse.error(str(e))
    
    def parse_response(self, content: str) -> AgentResponse:
        # Remove think blocks (Qwen models)
        import re
        content = re.sub(r'<think>.*?</think>', '', content, flags=re.DOTALL)
        
        # Find JSON
        start = content.find('{')
        end = content.rfind('}') + 1
        
        if start < 0 or end <= start:
            logger.warning("No JSON found in response")
            return AgentResponse(
                analysis="Failed to parse response",
                plan=content[:500],
                commands=[],
                task_complete=False
            )
        
        try:
            data = json.loads(content[start:end])
            
            commands = []
            for cmd in data.get("commands", []):
                if isinstance(cmd, dict):
                    commands.append(Command(
                        cmd.get("keystrokes", ""),
                        cmd.get("duration", 1.0)
                    ))
                elif isinstance(cmd, str):
                    commands.append(Command(cmd))
            
            return AgentResponse(
                analysis=data.get("analysis", ""),
                plan=data.get("plan", ""),
                commands=commands,
                task_complete=data.get("task_complete", False)
            )
            
        except json.JSONDecodeError as e:
            logger.warning(f"JSON parse error: {e}")
            return AgentResponse(
                analysis=f"JSON parse error: {e}",
                plan=content[:500],
                commands=[],
                task_complete=False
            )
    
    async def cleanup(self):
        if self.client:
            logger.info(f"Session stats: ${self.client.total_cost:.4f}, "
                       f"{self.client.total_tokens} tokens, "
                       f"{self.client.request_count} requests")


if __name__ == "__main__":
    import argparse
    
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", default="anthropic/claude-3-haiku")
    args, _ = parser.parse_known_args()
    
    run(TerminalAgent(model=args.model))
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `OPENROUTER_API_KEY` | OpenRouter API key |
| `CHUTES_API_KEY` | Chutes API key |
| `OPENAI_API_KEY` | OpenAI API key |
| `ANTHROPIC_API_KEY` | Anthropic API key |
| `LLM_API_KEY` | Generic fallback |
| `LLM_PROVIDER` | Default provider |
| `LLM_MODEL` | Default model |
