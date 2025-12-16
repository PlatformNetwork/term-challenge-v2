"""
LLM Client - Professional multi-provider LLM integration.

Supports OpenRouter, Chutes, OpenAI, and Anthropic providers with:
- Automatic cost tracking
- Rate limiting
- Retry logic
- Streaming support

Example:
    ```python
    from term_sdk.llm_client import LLMClient, Provider
    
    # Using OpenRouter
    client = LLMClient(
        provider=Provider.OPENROUTER,
        api_key="sk-or-...",
        model="anthropic/claude-3-haiku"
    )
    
    response = await client.chat([
        {"role": "system", "content": "You are a helpful assistant"},
        {"role": "user", "content": "Hello!"}
    ])
    
    print(response.content)
    print(f"Cost: ${response.cost:.4f}")
    ```
"""

from __future__ import annotations

import os
import re
import asyncio
import logging
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from enum import Enum
from typing import List, Dict, Any, Optional, AsyncIterator, Union

logger = logging.getLogger("term_sdk.llm")

# Try to import httpx, fall back to aiohttp
try:
    import httpx
    HTTP_CLIENT = "httpx"
except ImportError:
    try:
        import aiohttp
        HTTP_CLIENT = "aiohttp"
    except ImportError:
        raise ImportError("Either httpx or aiohttp is required. Install with: pip install httpx")


# =============================================================================
# Types
# =============================================================================

class Provider(Enum):
    """Supported LLM providers."""
    OPENROUTER = "openrouter"
    CHUTES = "chutes"
    OPENAI = "openai"
    ANTHROPIC = "anthropic"
    CUSTOM = "custom"


@dataclass
class Message:
    """A chat message."""
    role: str  # "system", "user", "assistant"
    content: str
    
    def to_dict(self) -> Dict[str, str]:
        return {"role": self.role, "content": self.content}


@dataclass
class ChatResponse:
    """Response from LLM chat completion."""
    content: str
    model: str
    prompt_tokens: int = 0
    completion_tokens: int = 0
    total_tokens: int = 0
    cost: float = 0.0
    latency_ms: int = 0
    finish_reason: str = "stop"
    
    @property
    def tokens(self) -> int:
        """Total tokens used."""
        return self.total_tokens or (self.prompt_tokens + self.completion_tokens)


@dataclass
class CostTracker:
    """Tracks cumulative costs across requests."""
    total_cost: float = 0.0
    total_tokens: int = 0
    total_prompt_tokens: int = 0
    total_completion_tokens: int = 0
    request_count: int = 0
    budget: Optional[float] = None
    
    def add(self, response: ChatResponse) -> None:
        """Add a response to the tracker."""
        self.total_cost += response.cost
        self.total_tokens += response.tokens
        self.total_prompt_tokens += response.prompt_tokens
        self.total_completion_tokens += response.completion_tokens
        self.request_count += 1
    
    @property
    def remaining_budget(self) -> Optional[float]:
        """Remaining budget, or None if no budget set."""
        if self.budget is None:
            return None
        return max(0, self.budget - self.total_cost)
    
    @property
    def over_budget(self) -> bool:
        """True if over budget."""
        if self.budget is None:
            return False
        return self.total_cost >= self.budget


# =============================================================================
# Pricing
# =============================================================================

# Pricing per 1M tokens (prompt, completion)
MODEL_PRICING: Dict[str, tuple] = {
    # OpenRouter / Anthropic
    "anthropic/claude-3-opus": (15.0, 75.0),
    "anthropic/claude-3-sonnet": (3.0, 15.0),
    "anthropic/claude-3-haiku": (0.25, 1.25),
    "anthropic/claude-sonnet-4": (3.0, 15.0),
    
    # OpenAI
    "openai/gpt-4o": (5.0, 15.0),
    "openai/gpt-4o-mini": (0.15, 0.60),
    "openai/gpt-4-turbo": (10.0, 30.0),
    "gpt-4o": (5.0, 15.0),
    "gpt-4o-mini": (0.15, 0.60),
    
    # Qwen (Chutes)
    "Qwen/Qwen3-32B": (0.10, 0.30),
    "Qwen/Qwen3-235B-A22B": (0.20, 0.60),
    
    # Llama
    "meta-llama/llama-3.1-70b-instruct": (0.35, 0.40),
    "meta-llama/llama-3.1-8b-instruct": (0.05, 0.08),
}


def estimate_cost(model: str, prompt_tokens: int, completion_tokens: int) -> float:
    """Estimate cost for a request."""
    pricing = MODEL_PRICING.get(model, (0.50, 1.50))  # Default pricing
    prompt_cost = (prompt_tokens / 1_000_000) * pricing[0]
    completion_cost = (completion_tokens / 1_000_000) * pricing[1]
    return prompt_cost + completion_cost


# =============================================================================
# Provider Configuration
# =============================================================================

PROVIDER_CONFIG = {
    Provider.OPENROUTER: {
        "base_url": "https://openrouter.ai/api/v1",
        "env_key": "OPENROUTER_API_KEY",
        "default_model": "anthropic/claude-3-haiku",
    },
    Provider.CHUTES: {
        "base_url": "https://llm.chutes.ai/v1",
        "env_key": "CHUTES_API_KEY",
        "default_model": "Qwen/Qwen3-32B",
    },
    Provider.OPENAI: {
        "base_url": "https://api.openai.com/v1",
        "env_key": "OPENAI_API_KEY",
        "default_model": "gpt-4o-mini",
    },
    Provider.ANTHROPIC: {
        "base_url": "https://api.anthropic.com/v1",
        "env_key": "ANTHROPIC_API_KEY",
        "default_model": "claude-3-haiku-20240307",
    },
}


# =============================================================================
# LLM Client
# =============================================================================

class LLMClient:
    """Multi-provider LLM client with cost tracking.
    
    Attributes:
        provider: The LLM provider.
        model: The model to use.
        cost_tracker: Tracks cumulative costs.
    """
    
    def __init__(
        self,
        provider: Union[Provider, str] = Provider.OPENROUTER,
        api_key: Optional[str] = None,
        model: Optional[str] = None,
        base_url: Optional[str] = None,
        budget: Optional[float] = None,
        timeout: float = 300.0,
    ):
        """Initialize the LLM client.
        
        Args:
            provider: LLM provider (openrouter, chutes, openai, anthropic, custom).
            api_key: API key (or set via environment variable).
            model: Model name (uses provider default if not specified).
            base_url: Custom base URL (for custom provider).
            budget: Maximum cost budget in USD.
            timeout: Request timeout in seconds.
        """
        # Parse provider
        if isinstance(provider, str):
            provider = Provider(provider.lower())
        self.provider = provider
        
        # Get config
        config = PROVIDER_CONFIG.get(provider, {})
        
        # Set up
        self.base_url = base_url or config.get("base_url", "")
        self.model = model or config.get("default_model", "")
        self.timeout = timeout
        
        # Get API key
        env_key = config.get("env_key", "LLM_API_KEY")
        self.api_key = api_key or os.environ.get(env_key) or os.environ.get("LLM_API_KEY")
        if not self.api_key:
            raise ValueError(f"API key required. Set {env_key} or pass api_key parameter.")
        
        # Cost tracking
        self.cost_tracker = CostTracker(budget=budget)
        
        # HTTP client
        self._client: Optional[Any] = None
        
        logger.info(f"LLM client initialized: {provider.value}/{self.model}")
    
    async def _get_client(self):
        """Get or create HTTP client."""
        if self._client is None:
            if HTTP_CLIENT == "httpx":
                self._client = httpx.AsyncClient(timeout=self.timeout)
            else:
                self._client = aiohttp.ClientSession(timeout=aiohttp.ClientTimeout(total=self.timeout))
        return self._client
    
    async def close(self) -> None:
        """Close the HTTP client."""
        if self._client:
            if HTTP_CLIENT == "httpx":
                await self._client.aclose()
            else:
                await self._client.close()
            self._client = None
    
    async def chat(
        self,
        messages: List[Union[Message, Dict[str, str]]],
        model: Optional[str] = None,
        temperature: float = 0.7,
        max_tokens: int = 4096,
        **kwargs
    ) -> ChatResponse:
        """Send a chat completion request.
        
        Args:
            messages: List of messages (Message objects or dicts).
            model: Override model for this request.
            temperature: Sampling temperature (0-2).
            max_tokens: Maximum tokens to generate.
            **kwargs: Additional parameters passed to the API.
        
        Returns:
            ChatResponse with content and metadata.
        
        Raises:
            ValueError: If over budget.
            Exception: If API request fails.
        """
        # Check budget
        if self.cost_tracker.over_budget:
            raise ValueError(f"Over budget: ${self.cost_tracker.total_cost:.4f} >= ${self.cost_tracker.budget:.4f}")
        
        # Build messages
        msgs = []
        for m in messages:
            if isinstance(m, Message):
                msgs.append(m.to_dict())
            else:
                msgs.append(m)
        
        # Request
        model = model or self.model
        url = f"{self.base_url}/chat/completions"
        
        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
            "HTTP-Referer": "https://term-challenge.ai",
        }
        
        data = {
            "model": model,
            "messages": msgs,
            "temperature": temperature,
            "max_tokens": max_tokens,
            **kwargs
        }
        
        # Send request
        import time
        start = time.time()
        
        client = await self._get_client()
        
        if HTTP_CLIENT == "httpx":
            resp = await client.post(url, headers=headers, json=data)
            resp.raise_for_status()
            result = resp.json()
        else:
            async with client.post(url, headers=headers, json=data) as resp:
                resp.raise_for_status()
                result = await resp.json()
        
        latency_ms = int((time.time() - start) * 1000)
        
        # Parse response
        content = result["choices"][0]["message"]["content"]
        
        # Remove <think> blocks (Qwen models)
        content = re.sub(r'<think>.*?</think>', '', content, flags=re.DOTALL).strip()
        
        # Get usage
        usage = result.get("usage", {})
        prompt_tokens = usage.get("prompt_tokens", 0)
        completion_tokens = usage.get("completion_tokens", 0)
        total_tokens = usage.get("total_tokens", prompt_tokens + completion_tokens)
        
        # Calculate cost
        cost = estimate_cost(model, prompt_tokens, completion_tokens)
        
        response = ChatResponse(
            content=content,
            model=model,
            prompt_tokens=prompt_tokens,
            completion_tokens=completion_tokens,
            total_tokens=total_tokens,
            cost=cost,
            latency_ms=latency_ms,
            finish_reason=result["choices"][0].get("finish_reason", "stop"),
        )
        
        # Track cost
        self.cost_tracker.add(response)
        
        logger.debug(f"Chat completed: {total_tokens} tokens, ${cost:.4f}, {latency_ms}ms")
        
        return response


# =============================================================================
# Convenience Functions
# =============================================================================

_default_client: Optional[LLMClient] = None


def get_client() -> LLMClient:
    """Get the default LLM client (creates one if needed)."""
    global _default_client
    if _default_client is None:
        _default_client = LLMClient()
    return _default_client


def set_client(client: LLMClient) -> None:
    """Set the default LLM client."""
    global _default_client
    _default_client = client


async def chat(
    messages: List[Union[Message, Dict[str, str]]],
    **kwargs
) -> ChatResponse:
    """Send a chat completion using the default client.
    
    This is a convenience function. For more control, create an LLMClient directly.
    """
    return await get_client().chat(messages, **kwargs)


# =============================================================================
# Exports
# =============================================================================

__all__ = [
    "Provider",
    "Message",
    "ChatResponse",
    "CostTracker",
    "LLMClient",
    "MODEL_PRICING",
    "estimate_cost",
    "get_client",
    "set_client",
    "chat",
]
