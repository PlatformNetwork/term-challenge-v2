"""
LLM Client for Term Challenge agents.

Supports OpenRouter and Chutes providers with streaming.

Example:
    ```python
    from term_sdk import LLM, LLMError
    
    llm = LLM()
    
    # Regular call
    result = llm.ask("Hello", model="z-ai/glm-4.5")
    
    # Streaming - see response in real-time
    for chunk in llm.stream("Write a story", model="z-ai/glm-4.5"):
        print(chunk, end="", flush=True)
    
    # Error handling
    try:
        result = llm.ask("Question", model="z-ai/glm-4.5")
    except LLMError as e:
        print(f"Code: {e.code}")
        print(f"Message: {e.message}")
        print(f"Details: {e.details}")
    ```
"""

import os
import sys
import json
import time
from dataclasses import dataclass, field
from typing import Optional, List, Dict, Any, Callable, Iterator, Union
import httpx

from .types import Tool, FunctionCall


class LLMError(Exception):
    """
    Structured LLM error with JSON details.
    
    Attributes:
        code: Error code (e.g., "rate_limit", "invalid_model", "api_error")
        message: Human-readable error message
        details: Additional error details as dict
    
    Example:
        ```python
        try:
            result = llm.ask("Question", model="invalid-model")
        except LLMError as e:
            print(f"Error {e.code}: {e.message}")
            print(f"Details: {e.details}")
        ```
    """
    def __init__(self, code: str, message: str, details: Optional[Dict[str, Any]] = None):
        self.code = code
        self.message = message
        self.details = details or {}
        super().__init__(self.to_json())
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "error": {
                "code": self.code,
                "message": self.message,
                "details": self.details,
            }
        }
    
    def to_json(self) -> str:
        return json.dumps(self.to_dict())
    
    def __str__(self) -> str:
        return f"LLMError({self.code}): {self.message}"


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
        return len(self.function_calls) > 0


def _log(msg: str):
    print(f"[llm] {msg}", file=sys.stderr)


# Provider configurations
PROVIDERS = {
    "openrouter": {
        "url": "https://openrouter.ai/api/v1/chat/completions",
        "env_key": "OPENROUTER_API_KEY",
        "description": "OpenRouter - Multi-model gateway",
    },
    "chutes": {
        "url": "https://llm.chutes.ai/v1/chat/completions",
        "env_key": "CHUTES_API_KEY",
        "description": "Chutes - Fast inference",
    },
    "openai": {
        "url": "https://api.openai.com/v1/chat/completions",
        "env_key": "OPENAI_API_KEY",
        "description": "OpenAI - GPT models",
    },
    "anthropic": {
        "url": "https://api.anthropic.com/v1/messages",
        "env_key": "ANTHROPIC_API_KEY",
        "description": "Anthropic - Claude models",
        "is_anthropic": True,
    },
    "grok": {
        "url": "https://api.x.ai/v1/chat/completions",
        "env_key": "GROK_API_KEY",
        "description": "xAI - Grok models",
    },
}

# Model pricing per 1M tokens (input, output)
PRICING = {
    # OpenAI
    "gpt-4o": (2.5, 10.0),
    "gpt-4o-mini": (0.15, 0.6),
    "gpt-4-turbo": (10.0, 30.0),
    "gpt-4": (30.0, 60.0),
    "gpt-3.5-turbo": (0.5, 1.5),
    # Anthropic
    "claude-3-opus": (15.0, 75.0),
    "claude-3-sonnet": (3.0, 15.0),
    "claude-3.5-sonnet": (3.0, 15.0),
    "claude-3-haiku": (0.25, 1.25),
    # Grok
    "grok-2": (2.0, 10.0),
    "grok-beta": (5.0, 15.0),
    # Open source
    "llama-3": (0.2, 0.2),
    "mixtral": (0.5, 0.5),
    "qwen": (0.2, 0.2),
    "glm-4": (0.25, 1.25),
}

# Default models per provider
DEFAULT_MODELS = {
    "openrouter": "anthropic/claude-3.5-sonnet",
    "chutes": "deepseek-ai/DeepSeek-V3-0324",
    "openai": "gpt-4o-mini",
    "anthropic": "claude-3-5-sonnet-20241022",
    "grok": "grok-2-latest",
}


class LLM:
    """
    LLM client with streaming support for multiple providers.
    
    Providers:
        - openrouter: Multi-model gateway (default)
        - chutes: Fast inference
        - openai: GPT models
        - anthropic: Claude models
        - grok: xAI Grok models
    
    Args:
        provider: Provider name (default: "openrouter")
        default_model: Default model if not specified per-call
        temperature: Default sampling temperature
        max_tokens: Default maximum response tokens
        timeout: Request timeout in seconds (default: 300, or LLM_TIMEOUT env var)
    
    Environment Variables:
        - OPENROUTER_API_KEY: OpenRouter API key
        - CHUTES_API_KEY: Chutes API key
        - OPENAI_API_KEY: OpenAI API key
        - ANTHROPIC_API_KEY: Anthropic API key
        - GROK_API_KEY: xAI Grok API key
        - LLM_API_KEY: Override for any provider
    
    Example:
        ```python
        # OpenRouter (default)
        llm = LLM()
        result = llm.ask("Question", model="anthropic/claude-3.5-sonnet")
        
        # OpenAI
        llm = LLM(provider="openai")
        result = llm.ask("Question", model="gpt-4o-mini")
        
        # Anthropic
        llm = LLM(provider="anthropic")
        result = llm.ask("Question", model="claude-3-5-sonnet-20241022")
        
        # Grok
        llm = LLM(provider="grok")
        result = llm.ask("Question", model="grok-2-latest")
        
        # Streaming
        for chunk in llm.stream("Long question"):
            print(chunk, end="", flush=True)
        ```
    """
    
    def __init__(
        self,
        provider: str = "openrouter",
        default_model: Optional[str] = None,
        temperature: float = 0.3,
        max_tokens: int = 4096,
        timeout: Optional[int] = None,
    ):
        self.provider = provider
        self.temperature = temperature
        self.max_tokens = max_tokens
        # Timeout: user param > env var > default 300s
        self.timeout = timeout or int(os.environ.get("LLM_TIMEOUT", "300"))
        
        # Get provider config
        if provider not in PROVIDERS:
            raise LLMError(
                code="invalid_provider",
                message=f"Unknown provider: {provider}",
                details={"valid_providers": list(PROVIDERS.keys())}
            )
        
        config = PROVIDERS[provider]
        self._api_url = os.environ.get("LLM_API_URL", config["url"])
        self._api_key = os.environ.get("LLM_API_KEY") or os.environ.get(config["env_key"], "")
        self._is_anthropic = config.get("is_anthropic", False)
        
        # Set default model (user > provider default)
        self.default_model = default_model or DEFAULT_MODELS.get(provider)
        
        if not self._api_key:
            _log(f"Warning: LLM_API_KEY or {config['env_key']} not set")
        
        # Stats
        self.stats: Dict[str, Dict[str, Any]] = {}
        self.total_tokens = 0
        self.total_cost = 0.0
        self.request_count = 0
        
        # HTTP client
        self._client = httpx.Client(timeout=timeout)
        
        # Function handlers
        self._function_handlers: Dict[str, Callable] = {}
    
    def _get_model(self, model: Optional[str]) -> str:
        if model:
            return model
        if self.default_model:
            return self.default_model
        raise LLMError(
            code="no_model",
            message="No model specified",
            details={"hint": "Pass model= parameter or set default_model in LLM()"}
        )
    
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
        """Ask a question (non-streaming)."""
        messages = []
        if system:
            messages.append({"role": "system", "content": system})
        messages.append({"role": "user", "content": prompt})
        return self.chat(messages, model=model, tools=tools, 
                        temperature=temperature, max_tokens=max_tokens)
    
    def stream(
        self,
        prompt: str,
        model: Optional[str] = None,
        system: Optional[str] = None,
        temperature: Optional[float] = None,
        max_tokens: Optional[int] = None,
    ) -> Iterator[str]:
        """
        Stream response chunks.
        
        Yields text chunks as they arrive.
        
        Example:
            ```python
            for chunk in llm.stream("Tell me a story", model="z-ai/glm-4.5"):
                print(chunk, end="", flush=True)
            ```
        """
        messages = []
        if system:
            messages.append({"role": "system", "content": system})
        messages.append({"role": "user", "content": prompt})
        yield from self.chat_stream(messages, model=model,
                                    temperature=temperature, max_tokens=max_tokens)
    
    def ask_stream(
        self,
        prompt: str,
        model: Optional[str] = None,
        system: Optional[str] = None,
        on_chunk: Optional[Callable[[str], bool]] = None,
        temperature: Optional[float] = None,
        max_tokens: Optional[int] = None,
    ) -> LLMResponse:
        """
        Stream with callback, return full response.
        
        Args:
            prompt: User prompt
            model: Model to use
            system: System prompt
            on_chunk: Callback for each chunk. Return False to stop.
            temperature: Sampling temperature
            max_tokens: Max tokens
        
        Returns:
            LLMResponse with full text
        
        Example:
            ```python
            def check(chunk):
                print(chunk, end="")
                return "ERROR" not in chunk  # Stop on error
            
            result = llm.ask_stream("Solve", model="z-ai/glm-4.5", on_chunk=check)
            ```
        """
        messages = []
        if system:
            messages.append({"role": "system", "content": system})
        messages.append({"role": "user", "content": prompt})
        return self.chat_stream_full(messages, model=model, on_chunk=on_chunk,
                                     temperature=temperature, max_tokens=max_tokens)
    
    def chat(
        self,
        messages: List[Dict[str, str]],
        model: Optional[str] = None,
        tools: Optional[List[Tool]] = None,
        temperature: Optional[float] = None,
        max_tokens: Optional[int] = None,
    ) -> LLMResponse:
        """Chat (non-streaming)."""
        model = self._get_model(model)
        temp = temperature if temperature is not None else self.temperature
        tokens = max_tokens if max_tokens is not None else self.max_tokens
        
        start = time.time()
        
        # Handle Anthropic's different API format
        if self._is_anthropic:
            return self._chat_anthropic(messages, model, tools, temp, tokens, start)
        
        payload: Dict[str, Any] = {
            "model": model,
            "messages": messages,
            "temperature": temp,
            "max_tokens": tokens,
            "stream": False,
        }
        
        if tools:
            payload["tools"] = [t.to_dict() for t in tools]
            payload["tool_choice"] = "auto"
        
        headers = {
            "Authorization": f"Bearer {self._api_key}",
            "Content-Type": "application/json",
        }
        
        response = self._client.post(self._api_url, headers=headers, json=payload)
        
        if not response.is_success:
            self._handle_api_error(response, model)
        
        data = response.json()
        
        return self._parse_response(data, model, start)
    
    def _chat_anthropic(
        self,
        messages: List[Dict[str, str]],
        model: str,
        tools: Optional[List[Tool]],
        temperature: float,
        max_tokens: int,
        start: float,
    ) -> LLMResponse:
        """Handle Anthropic's different API format."""
        # Extract system message if present
        system_content = None
        filtered_messages = []
        for msg in messages:
            if msg.get("role") == "system":
                system_content = msg.get("content", "")
            else:
                filtered_messages.append(msg)
        
        payload: Dict[str, Any] = {
            "model": model,
            "messages": filtered_messages,
            "temperature": temperature,
            "max_tokens": max_tokens,
        }
        
        if system_content:
            payload["system"] = system_content
        
        if tools:
            payload["tools"] = [self._convert_tool_to_anthropic(t) for t in tools]
        
        headers = {
            "x-api-key": self._api_key,
            "anthropic-version": "2023-06-01",
            "Content-Type": "application/json",
        }
        
        response = self._client.post(self._api_url, headers=headers, json=payload)
        
        if not response.is_success:
            self._handle_api_error(response, model)
        
        data = response.json()
        
        return self._parse_anthropic_response(data, model, start)
    
    def _convert_tool_to_anthropic(self, tool: Tool) -> Dict[str, Any]:
        """Convert OpenAI tool format to Anthropic format."""
        return {
            "name": tool.name,
            "description": tool.description,
            "input_schema": tool.parameters,
        }
    
    def _parse_anthropic_response(self, data: Dict, model: str, start: float) -> LLMResponse:
        """Parse Anthropic API response."""
        content = data.get("content", [])
        text = ""
        function_calls = []
        
        for block in content:
            if block.get("type") == "text":
                text += block.get("text", "")
            elif block.get("type") == "tool_use":
                function_calls.append(FunctionCall(
                    name=block.get("name", ""),
                    arguments=block.get("input", {}),
                    id=block.get("id"),
                ))
        
        usage = data.get("usage", {})
        prompt_tokens = usage.get("input_tokens", 0)
        completion_tokens = usage.get("output_tokens", 0)
        total_tokens = prompt_tokens + completion_tokens
        
        cost = self._calculate_cost(model, prompt_tokens, completion_tokens)
        latency_ms = int((time.time() - start) * 1000)
        
        self.total_tokens += total_tokens
        self.total_cost += cost
        self.request_count += 1
        self._update_model_stats(model, total_tokens, cost)
        
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
    
    def chat_stream(
        self,
        messages: List[Dict[str, str]],
        model: Optional[str] = None,
        temperature: Optional[float] = None,
        max_tokens: Optional[int] = None,
    ) -> Iterator[str]:
        """Stream chat response chunks."""
        model = self._get_model(model)
        temp = temperature if temperature is not None else self.temperature
        tokens = max_tokens if max_tokens is not None else self.max_tokens
        
        payload = {
            "model": model,
            "messages": messages,
            "temperature": temp,
            "max_tokens": tokens,
            "stream": True,
        }
        
        headers = {
            "Authorization": f"Bearer {self._api_key}",
            "Content-Type": "application/json",
        }
        
        with self._client.stream("POST", self._api_url, headers=headers, json=payload) as response:
            if not response.is_success:
                self._handle_api_error(response, model)
            for line in response.iter_lines():
                if line.startswith("data: "):
                    data = line[6:]
                    if data == "[DONE]":
                        break
                    try:
                        chunk = json.loads(data)
                        delta = chunk.get("choices", [{}])[0].get("delta", {})
                        content = delta.get("content", "")
                        if content:
                            yield content
                    except json.JSONDecodeError:
                        pass
    
    def chat_stream_full(
        self,
        messages: List[Dict[str, str]],
        model: Optional[str] = None,
        on_chunk: Optional[Callable[[str], bool]] = None,
        temperature: Optional[float] = None,
        max_tokens: Optional[int] = None,
    ) -> LLMResponse:
        """Stream and collect full response."""
        model = self._get_model(model)
        start = time.time()
        full_text = ""
        
        for chunk in self.chat_stream(messages, model=model, 
                                      temperature=temperature, max_tokens=max_tokens):
            full_text += chunk
            if on_chunk and not on_chunk(chunk):
                break
        
        latency_ms = int((time.time() - start) * 1000)
        
        # Estimate tokens (actual count not available in streaming)
        est_tokens = len(full_text) // 4
        cost = self._calculate_cost(model, est_tokens // 2, est_tokens // 2)
        
        self.total_tokens += est_tokens
        self.total_cost += cost
        self.request_count += 1
        
        self._update_model_stats(model, est_tokens, cost)
        
        return LLMResponse(
            text=full_text,
            model=model,
            tokens=est_tokens,
            cost=cost,
            latency_ms=latency_ms,
        )
    
    def chat_with_functions(
        self,
        messages: List[Dict[str, str]],
        tools: List[Tool],
        model: Optional[str] = None,
        max_iterations: int = 10,
        temperature: Optional[float] = None,
        max_tokens: Optional[int] = None,
    ) -> LLMResponse:
        """Chat with automatic function execution."""
        messages = list(messages)
        
        for _ in range(max_iterations):
            response = self.chat(messages, model=model, tools=tools,
                               temperature=temperature, max_tokens=max_tokens)
            
            if not response.function_calls:
                return response
            
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
    
    def execute_function(self, call: FunctionCall) -> Any:
        """Execute a registered function."""
        if call.name not in self._function_handlers:
            raise LLMError(
                code="unknown_function",
                message=f"Function '{call.name}' not registered",
                details={"registered_functions": list(self._function_handlers.keys())}
            )
        return self._function_handlers[call.name](**call.arguments)
    
    def _handle_api_error(self, response: httpx.Response, model: str):
        """Parse API error and raise LLMError with details."""
        status = response.status_code
        
        # Try to parse error from response body
        try:
            body = response.json()
            error_info = body.get("error", {})
            error_message = error_info.get("message", response.text[:200])
            error_type = error_info.get("type", "api_error")
        except:
            error_message = response.text[:200] if response.text else "Unknown error"
            error_type = "api_error"
        
        # Map HTTP status to error code
        if status == 401:
            code = "authentication_error"
            message = "Invalid API key"
        elif status == 403:
            code = "permission_denied"
            message = "Access denied for this model or endpoint"
        elif status == 404:
            code = "not_found"
            message = f"Model '{model}' not found"
        elif status == 429:
            code = "rate_limit"
            message = "Rate limit exceeded"
        elif status == 500:
            code = "server_error"
            message = "Provider server error"
        elif status == 503:
            code = "service_unavailable"
            message = "Provider service temporarily unavailable"
        else:
            code = error_type
            message = error_message
        
        raise LLMError(
            code=code,
            message=message,
            details={
                "http_status": status,
                "model": model,
                "provider": self.provider,
                "raw_error": error_message,
            }
        )
    
    def _parse_response(self, data: Dict, model: str, start: float) -> LLMResponse:
        choice = data.get("choices", [{}])[0]
        message = choice.get("message", {})
        text = message.get("content", "") or ""
        
        function_calls = []
        for tc in message.get("tool_calls", []) or []:
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
        
        usage = data.get("usage", {})
        prompt_tokens = usage.get("prompt_tokens", 0)
        completion_tokens = usage.get("completion_tokens", 0)
        total_tokens = prompt_tokens + completion_tokens
        
        cost = self._calculate_cost(model, prompt_tokens, completion_tokens)
        latency_ms = int((time.time() - start) * 1000)
        
        self.total_tokens += total_tokens
        self.total_cost += cost
        self.request_count += 1
        self._update_model_stats(model, total_tokens, cost)
        
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
    
    def _update_model_stats(self, model: str, tokens: int, cost: float):
        if model not in self.stats:
            self.stats[model] = {"tokens": 0, "cost": 0.0, "requests": 0}
        self.stats[model]["tokens"] += tokens
        self.stats[model]["cost"] += cost
        self.stats[model]["requests"] += 1
    
    def _calculate_cost(self, model: str, prompt_tokens: int, completion_tokens: int) -> float:
        input_price, output_price = 0.5, 1.5
        for key, prices in PRICING.items():
            if key in model.lower():
                input_price, output_price = prices
                break
        return (prompt_tokens * input_price + completion_tokens * output_price) / 1_000_000
    
    def get_stats(self, model: Optional[str] = None) -> Dict[str, Any]:
        """Get usage stats."""
        if model:
            return self.stats.get(model, {"tokens": 0, "cost": 0.0, "requests": 0})
        return {
            "total_tokens": self.total_tokens,
            "total_cost": self.total_cost,
            "request_count": self.request_count,
            "per_model": self.stats,
        }
    
    def close(self):
        self._client.close()
    
    def __enter__(self):
        return self
    
    def __exit__(self, *args):
        self.close()
