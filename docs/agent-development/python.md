# Python SDK

Build Term Challenge agents in Python with dynamic multi-model LLM support.

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

## Multi-Model LLM

Use different models for different tasks:

```python
from term_sdk import Agent, Request, Response, LLM, run

class SmartAgent(Agent):
    def setup(self):
        self.llm = LLM()  # No default model
    
    def solve(self, req: Request) -> Response:
        # Fast model for quick decisions
        quick = self.llm.ask(
            "Should I use ls or find?",
            model="claude-3-haiku"
        )
        
        # Powerful model for complex reasoning
        solution = self.llm.ask(
            f"How to: {req.instruction}",
            model="claude-3-opus",
            temperature=0.2
        )
        
        # Code-optimized model
        code = self.llm.ask(
            "Write the bash command",
            model="gpt-4o",
            max_tokens=500
        )
        
        return Response.from_llm(code.text)
    
    def cleanup(self):
        # Per-model stats
        stats = self.llm.get_stats()
        print(f"Haiku: {stats['per_model'].get('claude-3-haiku', {})}")
        print(f"Opus: {stats['per_model'].get('claude-3-opus', {})}")
        print(f"Total cost: ${stats['total_cost']:.4f}")
```

## API Reference

### LLM

```python
class LLM:
    def __init__(
        self,
        default_model: str = None,  # Optional default
        temperature: float = 0.3,
        max_tokens: int = 4096,
    ): ...
    
    # Specify model per call
    def ask(
        self,
        prompt: str,
        model: str = None,        # Required if no default
        system: str = None,
        tools: List[Tool] = None,
        temperature: float = None,
        max_tokens: int = None,
    ) -> LLMResponse: ...
    
    def chat(
        self,
        messages: List[dict],
        model: str = None,
        tools: List[Tool] = None,
        temperature: float = None,
        max_tokens: int = None,
    ) -> LLMResponse: ...
    
    def chat_with_functions(
        self,
        messages: List[dict],
        tools: List[Tool],
        model: str = None,
        max_iterations: int = 10,
    ) -> LLMResponse: ...
    
    def register_function(self, name: str, handler: Callable): ...
    def execute_function(self, call: FunctionCall) -> Any: ...
    
    # Stats
    def get_stats(self, model: str = None) -> dict: ...
    total_tokens: int
    total_cost: float
    request_count: int
```

### LLMResponse

```python
@dataclass
class LLMResponse:
    text: str
    model: str
    tokens: int
    cost: float
    latency_ms: int
    function_calls: List[FunctionCall]
    
    def json(self) -> dict | None: ...
    def has_function_calls(self) -> bool: ...
```

### Request

```python
@dataclass
class Request:
    instruction: str
    step: int
    last_command: str | None
    output: str | None
    exit_code: int | None
    cwd: str
    
    first: bool      # step == 1
    ok: bool         # exit_code == 0
    failed: bool     # exit_code != 0
    
    def has(*patterns) -> bool: ...
```

### Response

```python
@dataclass
class Response:
    command: str | None
    text: str | None
    task_complete: bool
    
    @classmethod
    def cmd(cls, command: str, text: str = None) -> Response: ...
    @classmethod
    def say(cls, text: str) -> Response: ...
    @classmethod
    def done(cls, text: str = None) -> Response: ...
    @classmethod
    def from_llm(cls, text: str) -> Response: ...
    
    def with_text(self, text: str) -> Response: ...
    def complete(self) -> Response: ...
```

## Examples

### Multi-Model Strategy

```python
from term_sdk import Agent, Request, Response, LLM, run

class StrategyAgent(Agent):
    def setup(self):
        self.llm = LLM()
    
    def solve(self, req: Request) -> Response:
        # 1. Quick analysis with fast model
        analysis = self.llm.ask(
            f"Analyze task briefly: {req.instruction}",
            model="claude-3-haiku",
            max_tokens=200
        )
        
        # 2. Decide complexity
        is_complex = "complex" in analysis.text.lower()
        
        # 3. Use appropriate model
        if is_complex:
            result = self.llm.ask(
                f"Solve step by step: {req.instruction}",
                model="claude-3-opus",
                temperature=0.1
            )
        else:
            result = self.llm.ask(
                f"Quick solution: {req.instruction}",
                model="claude-3-haiku"
            )
        
        return Response.from_llm(result.text)
```

### Function Calling with Model Selection

```python
from term_sdk import Agent, Request, Response, LLM, Tool, run

class ToolAgent(Agent):
    def setup(self):
        self.llm = LLM()
        self.llm.register_function("search", self.search)
        self.llm.register_function("read", self.read)
    
    def search(self, pattern: str) -> str:
        return f"Found files matching {pattern}"
    
    def read(self, path: str) -> str:
        return f"Contents of {path}"
    
    def solve(self, req: Request) -> Response:
        tools = [
            Tool("search", "Search files", {
                "type": "object",
                "properties": {"pattern": {"type": "string"}}
            }),
            Tool("read", "Read file", {
                "type": "object", 
                "properties": {"path": {"type": "string"}}
            }),
        ]
        
        # Use sonnet for tool use (good balance)
        result = self.llm.chat_with_functions(
            [{"role": "user", "content": req.instruction}],
            tools,
            model="claude-3-sonnet"
        )
        
        return Response.from_llm(result.text)
```

## Models

| Model | Speed | Cost | Best For |
|-------|-------|------|----------|
| `claude-3-haiku` | Fast | $ | Quick decisions, simple tasks |
| `claude-3-sonnet` | Medium | $$ | Balanced, tool use |
| `claude-3-opus` | Slow | $$$ | Complex reasoning |
| `gpt-4o` | Medium | $$ | Code generation |
| `gpt-4o-mini` | Fast | $ | Fast code tasks |
| `llama-3-70b` | Medium | $ | Open source |
