# Term SDK for Python

Build AI agents for the Terminal Benchmark Challenge.

## Installation

```bash
pip install term-sdk
```

Or install from source:

```bash
cd sdk/python
pip install -e .
```

## Quick Start

```python
from term_sdk import Agent, llm, terminal

@Agent.register
class MyAgent:
    async def solve(self, task: str) -> str:
        # Get current state
        result = await terminal.run("ls -la")
        
        # Ask LLM for next action
        response = await llm.chat(
            messages=[
                {"role": "system", "content": "You are a terminal expert."},
                {"role": "user", "content": f"Task: {task}\nCurrent dir:\n{result.output}"}
            ],
            model="openai/gpt-4o-mini"
        )
        
        # Execute the suggested command
        await terminal.run(response.content)
        return "Task completed"
```

## Configuration

Set your API key via environment variable:

```bash
# For OpenRouter (default)
export OPENROUTER_API_KEY=your-key

# For Chutes
export CHUTES_API_KEY=your-key
```

Or configure programmatically:

```python
from term_sdk import llm

llm.configure(
    provider="openrouter",  # or "chutes"
    api_key="your-key",
    cost_limit=10.0  # USD
)
```

## LLM Usage

### Simple Chat

```python
from term_sdk import llm

response = await llm.chat(
    messages=[{"role": "user", "content": "Hello!"}],
    model="openai/gpt-4o-mini"
)

print(response.content)
print(f"Cost: ${response.cost:.4f}")
print(f"Tokens: {response.total_tokens}")
```

### With System Prompt

```python
response = await llm.chat(
    messages=[
        {"role": "system", "content": "You are a bash expert."},
        {"role": "user", "content": "How do I list files?"}
    ],
    model="anthropic/claude-3-haiku",
    temperature=0.3
)
```

### Cost Tracking

```python
from term_sdk import llm

# Check remaining budget
print(f"Remaining: ${llm.remaining_budget:.2f}")

# Check total spent
print(f"Total spent: ${llm.total_cost:.4f}")
```

## Terminal Interface

The SDK provides a terminal interface compatible with terminal-bench's execution harness.

### Running Commands

```python
from term_sdk import terminal

# Run a command and get output
result = await terminal.run("ls -la")
print(result.output)
print(f"Duration: {result.duration_sec}s")
print(f"Timed out: {result.timed_out}")

# Non-blocking command (don't wait for output)
await terminal.run("sleep 5 &", block=False)

# Custom timeout
await terminal.run("make build", timeout_sec=300)
```

### Sending Keystrokes

For interactive programs (vim, less, etc.):

```python
# Open vim and edit a file
await terminal.send_keys("vim test.py", "Enter")
await terminal.send_keys("i")  # Enter insert mode
await terminal.send_keys("print('hello')")
await terminal.send_keys("Escape", ":wq", "Enter")

# Exit less/more
await terminal.send_keys("q")

# Cancel a command
await terminal.send_keys("Ctrl-C")
```

### Special Keys

| Key | Usage |
|-----|-------|
| `Enter` | Execute command |
| `Escape` | Exit mode (vim) |
| `Tab` | Autocomplete |
| `Ctrl-C` | Cancel/interrupt |
| `Ctrl-D` | EOF/exit |
| `Ctrl-Z` | Suspend |
| `Ctrl-L` | Clear screen |
| `Up`/`Down` | History navigation |
| `Backspace` | Delete char |

### Capturing Screen

```python
# Get current visible screen
screen = await terminal.capture_screen()

# Get full scrollback history
history = await terminal.capture_screen(full_history=True)

# Get new output since last call
output = await terminal.get_output()
```

### Example: Interactive Session

```python
from term_sdk import Agent, llm, terminal

@Agent.register
class VimAgent:
    async def solve(self, task: str) -> str:
        # Create a Python file using vim
        await terminal.send_keys("vim solution.py", "Enter")
        await terminal.wait(0.5)
        
        # Ask LLM for code
        response = await llm.chat(
            messages=[{"role": "user", "content": task}],
            model="openai/gpt-4o-mini"
        )
        
        # Enter insert mode and type code
        await terminal.send_keys("i")
        await terminal.send_keys(response.content)
        
        # Save and exit
        await terminal.send_keys("Escape", ":wq", "Enter")
        
        # Run the code
        result = await terminal.run("python solution.py")
        return result.output
```

## Available Models

### OpenAI
- `openai/gpt-4o` - Latest GPT-4 Omni ($2.50/$10.00 per 1M tokens)
- `openai/gpt-4o-mini` - Fast & cheap ($0.15/$0.60 per 1M tokens) **Recommended**
- `openai/o1-preview` - Reasoning model ($15.00/$60.00 per 1M tokens)
- `openai/o1-mini` - Fast reasoning ($3.00/$12.00 per 1M tokens)

### Anthropic
- `anthropic/claude-3.5-sonnet` - Best quality ($3.00/$15.00 per 1M tokens)
- `anthropic/claude-3-haiku` - Fast & cheap ($0.25/$1.25 per 1M tokens) **Recommended**
- `anthropic/claude-3-opus` - Most capable ($15.00/$75.00 per 1M tokens)

### Meta (Llama)
- `meta-llama/llama-3.1-70b-instruct` ($0.52/$0.75 per 1M tokens)
- `meta-llama/llama-3.1-8b-instruct` ($0.055/$0.055 per 1M tokens)

### Others
- `mistralai/mixtral-8x7b-instruct`
- `google/gemini-pro`
- `deepseek/deepseek-chat`

## Decorators

### Retry on Failure

```python
from term_sdk import with_retry

@with_retry(max_attempts=3, delay=1.0)
async def unreliable_operation():
    ...
```

### Rate Limiting

```python
from term_sdk import rate_limit

@rate_limit(calls=10, period=60)  # 10 calls per minute
async def api_call():
    ...
```

### Tools

```python
from term_sdk import Agent, tool

class MyAgent(Agent):
    @tool(description="Execute a shell command")
    async def run_command(self, cmd: str) -> str:
        # Tool implementation
        ...
    
    async def solve(self, task: str) -> str:
        result = await self.run_command("ls -la")
        ...
```

## Cost Limits

The default cost limit is **$10.00**. If exceeded, a `CostLimitExceeded` exception is raised.

```python
from term_sdk import CostLimitExceeded

try:
    response = await llm.chat(messages=[...])
except CostLimitExceeded as e:
    print(f"Budget exceeded: {e}")
```

## Providers

### OpenRouter (Default)

```python
from term_sdk import OpenRouterProvider

provider = OpenRouterProvider(api_key="your-key")
response = await provider.chat(
    messages=[{"role": "user", "content": "Hi"}],
    model="openai/gpt-4o-mini"
)
```

### Chutes

```python
from term_sdk import ChutesProvider

provider = ChutesProvider(api_key="your-key")
response = await provider.chat(
    messages=[{"role": "user", "content": "Hi"}],
    model="gpt-4o-mini"
)
```

## Complete Example

```python
from term_sdk import Agent, agent, llm, with_retry

@agent(name="TerminalExpert", description="Expert at terminal tasks")
class TerminalAgent:
    def __init__(self):
        self.context = []
    
    async def setup(self):
        """Called before evaluation starts"""
        print("Agent initialized")
    
    async def teardown(self):
        """Called after evaluation ends"""
        print(f"Total cost: ${self.get_cost():.4f}")
    
    @with_retry(max_attempts=2)
    async def solve(self, task: str) -> str:
        self.context.append({"role": "user", "content": task})
        
        response = await llm.chat(
            messages=[
                {"role": "system", "content": "You are an expert at terminal tasks."},
                *self.context
            ],
            model="openai/gpt-4o-mini",
            temperature=0.2
        )
        
        self.context.append({"role": "assistant", "content": response.content})
        return response.content

# Run the agent
async def main():
    agent = TerminalAgent()
    await agent.setup()
    
    result = await agent.solve("List all Python files in the current directory")
    print(result)
    
    await agent.teardown()

if __name__ == "__main__":
    import asyncio
    asyncio.run(main())
```

## License

Apache-2.0
