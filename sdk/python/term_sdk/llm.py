"""
LLM Client for Term Challenge agents.

Agents can use multiple models dynamically - specify the model on each call.

Example:
    ```python
    from term_sdk import LLM
    
    llm = LLM()  # No default model needed
    
    # Use different models for different tasks
    result = llm.ask("Quick question", model="claude-3-haiku")
    result = llm.ask("Complex analysis", model="claude-3-opus")
    result = llm.ask("Code generation", model="gpt-4o")
    
    # With function calling
    result = llm.chat(messages, tools=tools, model="claude-3-sonnet")
    ```
"""

import os
import sys
import json
import time
from dataclasses import dataclass, field
from typing import Optional, List, Dict, Any, Callable
import httpx

from .types import Tool, FunctionCall


@dataclass
class LLMResponse:
    """Response from LLM."""
    text: str
    model: str
    tokens: int = 0
    cost: float = 0.0
    latency_ms: int = 0
    function_calls: List[FunctionCall] = field(default_factory=list)
    raw: Optional[Dict[str, Any]] = None
    
    def json(self) -> Optional[Dict]:
        """Parse response text as JSON."""
        try:
            text = self.text.strip()
            if "```" in text:
                import re
                match = re.search(r'```(?:json)?\s*(\{.*?\})\s*```', text, re.DOTALL)
                if match:
                    text = match.group(1)
            start = text.find('{')
            end = text.rfind('}')
            if start >= 0 and end > start:
                return json.loads(text[start:end + 1])
        except:
            pass
        return None
    
    def has_function_calls(self) -> bool:
        """Check if response has function calls."""
        return len(self.function_calls) > 0


def _log(msg: str):
    print(f"[llm] {msg}", file=sys.stderr)


class LLM:
    """
    LLM client for inference with dynamic model selection.
    
    Agents can use multiple models - specify the model on each call.
    The provider is determined at runtime based on environment configuration.
    
    Args:
        default_model: Default model if not specified per-call (optional)
        temperature: Default sampling temperature (0.0 - 2.0)
        max_tokens: Default maximum response tokens
        timeout: Request timeout in seconds
    
    Example:
        ```python
        llm = LLM()
        
        # Different models for different tasks
        quick = llm.ask("What is 2+2?", model="claude-3-haiku")
        detailed = llm.ask("Explain quantum physics", model="claude-3-opus")
        code = llm.ask("Write a Python function", model="gpt-4o")
        
        # Override parameters per call
        result = llm.ask(
            "Creative story",
            model="claude-3-sonnet",
            temperature=0.9,
            max_tokens=2000
        )
        
        # Chat with specific model
        result = llm.chat(
            messages=[{"role": "user", "content": "Hello"}],
            model="gpt-4o-mini"
        )
        ```
    """
    
    def __init__(
        self,
        default_model: Optional[str] = None,
        temperature: float = 0.3,
        max_tokens: int = 4096,
        timeout: int = 120,
    ):
        self.default_model = default_model
        self.temperature = temperature
        self.max_tokens = max_tokens
        self.timeout = timeout
        
        # Get API configuration from environment
        self._api_url = os.environ.get("LLM_API_URL", "https://openrouter.ai/api/v1/chat/completions")
        self._api_key = os.environ.get("LLM_API_KEY") or os.environ.get("OPENROUTER_API_KEY", "")
        
        if not self._api_key:
            _log("Warning: LLM_API_KEY or OPENROUTER_API_KEY not set")
        
        # Stats per model
        self.stats: Dict[str, Dict[str, Any]] = {}
        self.total_tokens = 0
        self.total_cost = 0.0
        self.request_count = 0
        
        # HTTP client
        self._client = httpx.Client(timeout=timeout)
        
        # Function handlers
        self._function_handlers: Dict[str, Callable] = {}
    
    def _get_model(self, model: Optional[str]) -> str:
        """Get model to use, with fallback to default."""
        if model:
            return model
        if self.default_model:
            return self.default_model
        raise ValueError("No model specified. Pass model= parameter or set default_model.")
    
    def register_function(self, name: str, handler: Callable):
        """Register a function handler for function calling."""
        self._function_handlers[name] = handler
    
    def ask(
        self,
        prompt: str,
        model: Optional[str] = None,
        system: Optional[str] = None,
        tools: Optional[List[Tool]] = None,
        temperature: Optional[float] = None,
        max_tokens: Optional[int] = None,
    ) -> LLMResponse:
        """
        Ask a question with specified model.
        
        Args:
            prompt: User prompt
            model: Model to use (required if no default_model)
            system: Optional system prompt
            tools: Optional list of tools/functions
            temperature: Override default temperature
            max_tokens: Override default max_tokens
        
        Returns:
            LLMResponse with text, function_calls, tokens, cost
        """
        messages = []
        if system:
            messages.append({"role": "system", "content": system})
        messages.append({"role": "user", "content": prompt})
        return self.chat(
            messages,
            model=model,
            tools=tools,
            temperature=temperature,
            max_tokens=max_tokens,
        )
    
    def chat(
        self,
        messages: List[Dict[str, str]],
        model: Optional[str] = None,
        tools: Optional[List[Tool]] = None,
        temperature: Optional[float] = None,
        max_tokens: Optional[int] = None,
    ) -> LLMResponse:
        """
        Chat with message history using specified model.
        
        Args:
            messages: List of {"role": "user/assistant/system", "content": "..."}
            model: Model to use (required if no default_model)
            tools: Optional list of tools/functions for function calling
            temperature: Override default temperature
            max_tokens: Override default max_tokens
        
        Returns:
            LLMResponse with text, function_calls, tokens, cost
        """
        model = self._get_model(model)
        temp = temperature if temperature is not None else self.temperature
        tokens = max_tokens if max_tokens is not None else self.max_tokens
        
        start = time.time()
        
        try:
            # Build request
            payload: Dict[str, Any] = {
                "model": model,
                "messages": messages,
                "temperature": temp,
                "max_tokens": tokens,
            }
            
            # Add tools if provided
            if tools:
                payload["tools"] = [t.to_dict() for t in tools]
                payload["tool_choice"] = "auto"
            
            # Make request
            headers = {
                "Authorization": f"Bearer {self._api_key}",
                "Content-Type": "application/json",
            }
            
            response = self._client.post(
                self._api_url,
                headers=headers,
                json=payload,
            )
            response.raise_for_status()
            data = response.json()
            
            # Parse response
            choice = data.get("choices", [{}])[0]
            message = choice.get("message", {})
            
            text = message.get("content", "") or ""
            
            # Parse function calls
            function_calls = []
            tool_calls = message.get("tool_calls", [])
            for tc in tool_calls:
                if tc.get("type") == "function":
                    func = tc.get("function", {})
                    try:
                        args = json.loads(func.get("arguments", "{}"))
                    except:
                        args = {}
                    function_calls.append(FunctionCall(
                        name=func.get("name", ""),
                        arguments=args,
                        id=tc.get("id"),
                    ))
            
            # Calculate tokens and cost
            usage = data.get("usage", {})
            prompt_tokens = usage.get("prompt_tokens", 0)
            completion_tokens = usage.get("completion_tokens", 0)
            total_tokens = prompt_tokens + completion_tokens
            
            cost = self._calculate_cost(model, prompt_tokens, completion_tokens)
            latency_ms = int((time.time() - start) * 1000)
            
            # Update stats
            self.total_tokens += total_tokens
            self.total_cost += cost
            self.request_count += 1
            
            # Per-model stats
            if model not in self.stats:
                self.stats[model] = {"tokens": 0, "cost": 0.0, "requests": 0}
            self.stats[model]["tokens"] += total_tokens
            self.stats[model]["cost"] += cost
            self.stats[model]["requests"] += 1
            
            _log(f"{model}: {total_tokens} tokens, ${cost:.4f}, {latency_ms}ms")
            
            return LLMResponse(
                text=text,
                model=model,
                tokens=total_tokens,
                cost=cost,
                latency_ms=latency_ms,
                function_calls=function_calls,
                raw=data,
            )
            
        except Exception as e:
            _log(f"Error: {e}")
            raise
    
    def execute_function(self, call: FunctionCall) -> Any:
        """Execute a registered function."""
        if call.name not in self._function_handlers:
            raise ValueError(f"Unknown function: {call.name}")
        return self._function_handlers[call.name](**call.arguments)
    
    def chat_with_functions(
        self,
        messages: List[Dict[str, str]],
        tools: List[Tool],
        model: Optional[str] = None,
        max_iterations: int = 10,
        temperature: Optional[float] = None,
        max_tokens: Optional[int] = None,
    ) -> LLMResponse:
        """
        Chat with automatic function execution.
        
        Automatically executes function calls and continues conversation
        until the model returns a text response.
        
        Args:
            messages: Initial messages
            tools: Available tools
            model: Model to use
            max_iterations: Max function call iterations
            temperature: Override default temperature
            max_tokens: Override default max_tokens
        
        Returns:
            Final LLMResponse
        """
        messages = list(messages)  # Copy
        
        for _ in range(max_iterations):
            response = self.chat(
                messages,
                model=model,
                tools=tools,
                temperature=temperature,
                max_tokens=max_tokens,
            )
            
            if not response.function_calls:
                return response
            
            # Execute functions and add results
            for call in response.function_calls:
                try:
                    result = self.execute_function(call)
                    messages.append({
                        "role": "assistant",
                        "content": None,
                        "tool_calls": [{
                            "id": call.id,
                            "type": "function",
                            "function": {
                                "name": call.name,
                                "arguments": json.dumps(call.arguments),
                            }
                        }]
                    })
                    messages.append({
                        "role": "tool",
                        "tool_call_id": call.id,
                        "content": json.dumps(result) if not isinstance(result, str) else result,
                    })
                except Exception as e:
                    messages.append({
                        "role": "tool",
                        "tool_call_id": call.id,
                        "content": f"Error: {e}",
                    })
        
        return response
    
    def get_stats(self, model: Optional[str] = None) -> Dict[str, Any]:
        """Get usage stats, optionally for a specific model."""
        if model:
            return self.stats.get(model, {"tokens": 0, "cost": 0.0, "requests": 0})
        return {
            "total_tokens": self.total_tokens,
            "total_cost": self.total_cost,
            "request_count": self.request_count,
            "per_model": self.stats,
        }
    
    def _calculate_cost(self, model: str, prompt_tokens: int, completion_tokens: int) -> float:
        """Calculate cost based on model pricing."""
        # Pricing per 1M tokens (input, output)
        pricing = {
            "claude-3-haiku": (0.25, 1.25),
            "claude-3-sonnet": (3.0, 15.0),
            "claude-3-opus": (15.0, 75.0),
            "claude-3.5-sonnet": (3.0, 15.0),
            "gpt-4o": (5.0, 15.0),
            "gpt-4o-mini": (0.15, 0.6),
            "gpt-4-turbo": (10.0, 30.0),
            "gpt-3.5-turbo": (0.5, 1.5),
            "llama-3": (0.2, 0.2),
            "mixtral": (0.5, 0.5),
        }
        
        # Find matching pricing
        input_price, output_price = 0.5, 1.5  # Default
        for key, prices in pricing.items():
            if key in model.lower():
                input_price, output_price = prices
                break
        
        return (prompt_tokens * input_price + completion_tokens * output_price) / 1_000_000
    
    def close(self):
        """Close HTTP client."""
        self._client.close()
    
    def __enter__(self):
        return self
    
    def __exit__(self, *args):
        self.close()
