"""
LLM Client for Term Challenge agents.

Supports multiple providers:
- OpenRouter (default): anthropic/claude-3-haiku, openai/gpt-4o, etc.
- OpenAI: gpt-4o, gpt-4o-mini, gpt-3.5-turbo
- Anthropic: claude-3-opus, claude-3-sonnet, claude-3-haiku

Example:
    ```python
    from term_sdk import LLM
    
    # OpenRouter (default)
    llm = LLM(model="anthropic/claude-3-haiku")
    
    # Direct OpenAI
    llm = LLM(provider="openai", model="gpt-4o")
    
    # Direct Anthropic
    llm = LLM(provider="anthropic", model="claude-3-haiku-20240307")
    
    # Ask a question
    response = llm.ask("What is 2+2?")
    print(response.text)
    
    # Chat with messages
    response = llm.chat([
        {"role": "system", "content": "You are helpful."},
        {"role": "user", "content": "Hello!"}
    ])
    ```
"""

import os
import sys
import json
import time
from dataclasses import dataclass
from typing import Optional, List, Dict, Any
import httpx


@dataclass
class LLMResponse:
    """Response from LLM."""
    text: str
    model: str
    tokens: int = 0
    cost: float = 0.0
    latency_ms: int = 0
    
    def json(self) -> Optional[Dict]:
        """Parse response as JSON."""
        try:
            text = self.text.strip()
            # Remove markdown code blocks
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


# Provider configurations
PROVIDERS = {
    "openrouter": {
        "url": "https://openrouter.ai/api/v1/chat/completions",
        "key_env": "OPENROUTER_API_KEY",
        "headers": lambda key: {
            "Authorization": f"Bearer {key}",
            "Content-Type": "application/json",
        }
    },
    "openai": {
        "url": "https://api.openai.com/v1/chat/completions",
        "key_env": "OPENAI_API_KEY",
        "headers": lambda key: {
            "Authorization": f"Bearer {key}",
            "Content-Type": "application/json",
        }
    },
    "anthropic": {
        "url": "https://api.anthropic.com/v1/messages",
        "key_env": "ANTHROPIC_API_KEY",
        "headers": lambda key: {
            "x-api-key": key,
            "Content-Type": "application/json",
            "anthropic-version": "2023-06-01",
        }
    },
}

# Model pricing (per 1M tokens) - input/output
PRICING = {
    # OpenRouter models
    "anthropic/claude-3-haiku": (0.25, 1.25),
    "anthropic/claude-3-sonnet": (3.0, 15.0),
    "anthropic/claude-3-opus": (15.0, 75.0),
    "anthropic/claude-3.5-sonnet": (3.0, 15.0),
    "openai/gpt-4o": (5.0, 15.0),
    "openai/gpt-4o-mini": (0.15, 0.6),
    "openai/gpt-3.5-turbo": (0.5, 1.5),
    "google/gemini-pro": (0.5, 1.5),
    "meta-llama/llama-3-70b": (0.8, 0.8),
    # Direct OpenAI
    "gpt-4o": (5.0, 15.0),
    "gpt-4o-mini": (0.15, 0.6),
    "gpt-3.5-turbo": (0.5, 1.5),
    # Direct Anthropic
    "claude-3-haiku-20240307": (0.25, 1.25),
    "claude-3-sonnet-20240229": (3.0, 15.0),
    "claude-3-opus-20240229": (15.0, 75.0),
}


def _log(msg: str):
    print(f"[llm] {msg}", file=sys.stderr)


class LLM:
    """
    LLM client for multiple providers.
    
    Args:
        provider: "openrouter" (default), "openai", or "anthropic"
        model: Model name (e.g., "anthropic/claude-3-haiku")
        api_key: API key (or set via environment variable)
        temperature: Sampling temperature (0.0 - 2.0)
        max_tokens: Maximum response tokens
        timeout: Request timeout in seconds
    
    Example:
        ```python
        llm = LLM(model="anthropic/claude-3-haiku")
        
        # Simple question
        response = llm.ask("What is Python?")
        print(response.text)
        
        # With system prompt
        response = llm.ask(
            "Write hello world",
            system="You are a Python expert."
        )
        
        # Chat with history
        response = llm.chat([
            {"role": "system", "content": "You are helpful."},
            {"role": "user", "content": "Hello!"},
            {"role": "assistant", "content": "Hi there!"},
            {"role": "user", "content": "How are you?"}
        ])
        ```
    """
    
    def __init__(
        self,
        provider: str = "openrouter",
        model: str = "anthropic/claude-3-haiku",
        api_key: Optional[str] = None,
        temperature: float = 0.3,
        max_tokens: int = 1024,
        timeout: int = 60,
    ):
        self.provider = provider
        self.model = model
        self.temperature = temperature
        self.max_tokens = max_tokens
        self.timeout = timeout
        
        # Get provider config
        if provider not in PROVIDERS:
            raise ValueError(f"Unknown provider: {provider}")
        self._config = PROVIDERS[provider]
        
        # Get API key
        self.api_key = api_key or os.environ.get(self._config["key_env"], "")
        if not self.api_key:
            _log(f"Warning: {self._config['key_env']} not set")
        
        # Stats
        self.total_tokens = 0
        self.total_cost = 0.0
        self.request_count = 0
        
        # HTTP client
        self._client = httpx.Client(timeout=timeout)
    
    def ask(
        self,
        prompt: str,
        system: Optional[str] = None,
        **kwargs
    ) -> LLMResponse:
        """
        Ask a simple question.
        
        Args:
            prompt: User prompt
            system: Optional system prompt
            **kwargs: Override model, temperature, max_tokens
        
        Returns:
            LLMResponse with text, tokens, cost
        """
        messages = []
        if system:
            messages.append({"role": "system", "content": system})
        messages.append({"role": "user", "content": prompt})
        return self.chat(messages, **kwargs)
    
    def chat(
        self,
        messages: List[Dict[str, str]],
        **kwargs
    ) -> LLMResponse:
        """
        Chat with message history.
        
        Args:
            messages: List of {"role": "user/assistant/system", "content": "..."}
            **kwargs: Override model, temperature, max_tokens
        
        Returns:
            LLMResponse with text, tokens, cost
        """
        model = kwargs.get("model", self.model)
        temperature = kwargs.get("temperature", self.temperature)
        max_tokens = kwargs.get("max_tokens", self.max_tokens)
        
        start = time.time()
        
        try:
            if self.provider == "anthropic":
                response = self._chat_anthropic(messages, model, temperature, max_tokens)
            else:
                response = self._chat_openai(messages, model, temperature, max_tokens)
            
            response.latency_ms = int((time.time() - start) * 1000)
            
            # Update stats
            self.total_tokens += response.tokens
            self.total_cost += response.cost
            self.request_count += 1
            
            _log(f"{model}: {response.tokens} tokens, ${response.cost:.4f}, {response.latency_ms}ms")
            return response
            
        except Exception as e:
            _log(f"Error: {e}")
            raise
    
    def _chat_openai(
        self,
        messages: List[Dict[str, str]],
        model: str,
        temperature: float,
        max_tokens: int,
    ) -> LLMResponse:
        """OpenAI/OpenRouter compatible API."""
        headers = self._config["headers"](self.api_key)
        
        payload = {
            "model": model,
            "messages": messages,
            "temperature": temperature,
            "max_tokens": max_tokens,
        }
        
        response = self._client.post(
            self._config["url"],
            headers=headers,
            json=payload,
        )
        response.raise_for_status()
        data = response.json()
        
        text = data["choices"][0]["message"]["content"]
        usage = data.get("usage", {})
        prompt_tokens = usage.get("prompt_tokens", 0)
        completion_tokens = usage.get("completion_tokens", 0)
        total_tokens = prompt_tokens + completion_tokens
        
        # Calculate cost
        pricing = PRICING.get(model, (0.5, 1.5))
        cost = (prompt_tokens * pricing[0] + completion_tokens * pricing[1]) / 1_000_000
        
        return LLMResponse(
            text=text,
            model=model,
            tokens=total_tokens,
            cost=cost,
        )
    
    def _chat_anthropic(
        self,
        messages: List[Dict[str, str]],
        model: str,
        temperature: float,
        max_tokens: int,
    ) -> LLMResponse:
        """Anthropic native API."""
        headers = self._config["headers"](self.api_key)
        
        # Extract system message
        system = None
        user_messages = []
        for msg in messages:
            if msg["role"] == "system":
                system = msg["content"]
            else:
                user_messages.append(msg)
        
        payload = {
            "model": model,
            "messages": user_messages,
            "temperature": temperature,
            "max_tokens": max_tokens,
        }
        if system:
            payload["system"] = system
        
        response = self._client.post(
            self._config["url"],
            headers=headers,
            json=payload,
        )
        response.raise_for_status()
        data = response.json()
        
        text = data["content"][0]["text"]
        usage = data.get("usage", {})
        prompt_tokens = usage.get("input_tokens", 0)
        completion_tokens = usage.get("output_tokens", 0)
        total_tokens = prompt_tokens + completion_tokens
        
        # Calculate cost
        pricing = PRICING.get(model, (0.25, 1.25))
        cost = (prompt_tokens * pricing[0] + completion_tokens * pricing[1]) / 1_000_000
        
        return LLMResponse(
            text=text,
            model=model,
            tokens=total_tokens,
            cost=cost,
        )
    
    def close(self):
        """Close HTTP client."""
        self._client.close()
    
    def __enter__(self):
        return self
    
    def __exit__(self, *args):
        self.close()
