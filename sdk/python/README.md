# Term SDK for Python

Build agents for Term Challenge.

## Installation

```bash
pip install -e sdk/python
```

## Quick Start

```python
from term_sdk import Agent, Request, Response, run

class MyAgent(Agent):
    def solve(self, req: Request) -> Response:
        if req.step == 1:
            return Response.cmd("ls -la")
        if req.has("hello"):
            return Response.done()
        return Response.cmd("echo hello")

if __name__ == "__main__":
    run(MyAgent())
```

## With LLM

```python
from term_sdk import Agent, Request, Response, LLM, run

class LLMAgent(Agent):
    def setup(self):
        self.llm = LLM(model="anthropic/claude-3-haiku")
    
    def solve(self, req: Request) -> Response:
        result = self.llm.ask(f"""
Task: {req.instruction}
Step: {req.step}
Last command: {req.last_command}
Output: {req.output}
Exit code: {req.exit_code}

Return JSON: {{"command": "...", "task_complete": false}}
""")
        return Response.from_llm(result.text)

if __name__ == "__main__":
    run(LLMAgent())
```

## API Reference

### Request

| Field | Type | Description |
|-------|------|-------------|
| `instruction` | str | Task to complete |
| `step` | int | Step number (1-indexed) |
| `last_command` | str? | Previous command |
| `output` | str? | Command output |
| `exit_code` | int? | Exit code |
| `cwd` | str | Working directory |

Properties:
- `req.first` - True on step 1
- `req.ok` - True if exit_code == 0
- `req.failed` - True if exit_code != 0
- `req.has("pattern")` - Check output contains pattern

### Response

```python
Response.cmd("ls -la")     # Execute command
Response.done()            # Task complete
Response.from_llm(text)    # Parse from LLM output
```

### LLM

```python
# OpenRouter (default)
llm = LLM(model="anthropic/claude-3-haiku")
llm = LLM(model="openai/gpt-4o")

# Direct providers
llm = LLM(provider="openai", model="gpt-4o")
llm = LLM(provider="anthropic", model="claude-3-haiku-20240307")

# Usage
response = llm.ask("question")
response = llm.ask("question", system="You are helpful.")
response = llm.chat([
    {"role": "user", "content": "Hello"}
])

print(response.text)    # Response text
print(response.tokens)  # Token count
print(response.cost)    # Cost in USD
```

## Environment Variables

- `OPENROUTER_API_KEY` - OpenRouter API key
- `OPENAI_API_KEY` - OpenAI API key  
- `ANTHROPIC_API_KEY` - Anthropic API key
