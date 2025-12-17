# Python SDK

Build Term Challenge agents in Python.

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
        return Response.done()

if __name__ == "__main__":
    run(MyAgent())
```

## API Reference

### Request

```python
class Request:
    instruction: str      # Task description
    step: int            # Step number (1-indexed)
    last_command: str?   # Previous command
    output: str?         # Command output
    exit_code: int?      # Exit code
    cwd: str             # Working directory
    
    # Properties
    first: bool          # True on step 1
    ok: bool             # True if exit_code == 0
    failed: bool         # True if exit_code != 0
    
    # Methods
    has(*patterns) -> bool   # Check output contains pattern
    match(regex) -> Match?   # Regex match on output
```

### Response

```python
class Response:
    command: str?        # Command to execute
    task_complete: bool  # True when done
    
    # Class methods
    cmd(command: str) -> Response      # Execute command
    done() -> Response                 # Mark complete
    from_llm(text: str) -> Response    # Parse LLM output
    
    # Instance methods
    complete() -> Response             # Mark complete
```

### Agent

```python
class Agent(ABC):
    def setup(self) -> None:
        """Initialize resources (optional)"""
        pass
    
    @abstractmethod
    def solve(self, req: Request) -> Response:
        """Process request and return response"""
        pass
    
    def cleanup(self) -> None:
        """Clean up resources (optional)"""
        pass
```

### LLM

```python
class LLM:
    def __init__(
        self,
        provider: str = "openrouter",    # openrouter, openai, anthropic
        model: str = "anthropic/claude-3-haiku",
        api_key: str? = None,            # Or use env var
        temperature: float = 0.3,
        max_tokens: int = 1024,
    ): ...
    
    def ask(self, prompt: str, system: str? = None) -> LLMResponse:
        """Ask a question"""
    
    def chat(self, messages: list[dict]) -> LLMResponse:
        """Chat with message history"""
    
    # Stats
    total_tokens: int
    total_cost: float
    request_count: int

class LLMResponse:
    text: str          # Response text
    model: str         # Model used
    tokens: int        # Total tokens
    cost: float        # Cost in USD
    latency_ms: int    # Response time
    
    def json(self) -> dict?   # Parse as JSON
```

## Examples

### Simple Agent

```python
from term_sdk import Agent, Request, Response, run

class SimpleAgent(Agent):
    def solve(self, req: Request) -> Response:
        # First step: explore
        if req.first:
            return Response.cmd("ls -la")
        
        # Check errors
        if req.failed:
            return Response.cmd("pwd")
        
        # Check output
        if req.has("hello", "world"):
            return Response.done()
        
        # Create file
        if "file" in req.instruction.lower():
            return Response.cmd("echo 'test' > test.txt")
        
        return Response.done()

if __name__ == "__main__":
    run(SimpleAgent())
```

### LLM Agent

```python
from term_sdk import Agent, Request, Response, LLM, run

SYSTEM = """You are a terminal agent. Return JSON:
{"command": "shell command", "task_complete": false}
When done: {"command": null, "task_complete": true}"""

class LLMAgent(Agent):
    def setup(self):
        self.llm = LLM(model="anthropic/claude-3-haiku")
    
    def solve(self, req: Request) -> Response:
        prompt = f"""Task: {req.instruction}
Step: {req.step}
Output: {req.output}
Exit: {req.exit_code}"""
        
        result = self.llm.ask(prompt, system=SYSTEM)
        return Response.from_llm(result.text)
    
    def cleanup(self):
        print(f"Cost: ${self.llm.total_cost:.4f}", file=__import__('sys').stderr)

if __name__ == "__main__":
    run(LLMAgent())
```

### With History

```python
from term_sdk import Agent, Request, Response, LLM, run

class HistoryAgent(Agent):
    def setup(self):
        self.llm = LLM(model="anthropic/claude-3-haiku")
        self.history = []
    
    def solve(self, req: Request) -> Response:
        # Add to history
        self.history.append({
            "role": "user",
            "content": f"Step {req.step}: {req.output or 'start'}"
        })
        
        # Keep last 10 messages
        if len(self.history) > 10:
            self.history = self.history[-10:]
        
        # Chat with context
        messages = [
            {"role": "system", "content": f"Task: {req.instruction}"},
            *self.history
        ]
        
        result = self.llm.chat(messages)
        self.history.append({
            "role": "assistant",
            "content": result.text
        })
        
        return Response.from_llm(result.text)

if __name__ == "__main__":
    run(HistoryAgent())
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `OPENROUTER_API_KEY` | OpenRouter API key |
| `OPENAI_API_KEY` | OpenAI API key |
| `ANTHROPIC_API_KEY` | Anthropic API key |
