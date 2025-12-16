"""
LLM client for inference with cost tracking
"""

import os
import asyncio
from dataclasses import dataclass, field
from typing import Any, Dict, List, Literal, Optional, Union

import httpx

from .cost import get_cost_tracker, MODEL_PRICING
from .exceptions import CostLimitExceeded, ProviderError


@dataclass
class Message:
    """Chat message"""
    role: Literal["system", "user", "assistant"]
    content: str
    name: Optional[str] = None
    
    def to_dict(self) -> Dict[str, str]:
        d = {"role": self.role, "content": self.content}
        if self.name:
            d["name"] = self.name
        return d


@dataclass
class ChatResponse:
    """Response from chat completion"""
    content: str
    model: str
    finish_reason: str
    usage: Dict[str, int]
    cost: float
    provider: str
    raw: Dict[str, Any] = field(default_factory=dict)
    
    @property
    def input_tokens(self) -> int:
        return self.usage.get("prompt_tokens", 0)
    
    @property
    def output_tokens(self) -> int:
        return self.usage.get("completion_tokens", 0)
    
    @property
    def total_tokens(self) -> int:
        return self.usage.get("total_tokens", 0)


class LLMClient:
    """
    Unified LLM client supporting multiple providers.
    
    Providers:
        - OpenRouter (default): https://openrouter.ai
        - Chutes: https://llm.chutes.ai
    
    Usage:
        from term_sdk import llm
        
        # Simple chat
        response = await llm.chat(
            messages=[{"role": "user", "content": "Hello!"}],
            model="openai/gpt-4o-mini"
        )
        print(response.content)
        
        # With system prompt
        response = await llm.chat(
            messages=[
                {"role": "system", "content": "You are a helpful assistant."},
                {"role": "user", "content": "Hello!"}
            ],
            model="anthropic/claude-3-haiku"
        )
    """
    
    PROVIDERS = {
        "openrouter": {
            "url": "https://openrouter.ai/api/v1/chat/completions",
            "env_key": "OPENROUTER_API_KEY",
        },
        "chutes": {
            "url": "https://llm.chutes.ai/v1/chat/completions",
            "env_key": "CHUTES_API_KEY",
        },
    }
    
    def __init__(
        self,
        provider: str = "openrouter",
        api_key: Optional[str] = None,
        cost_limit: float = 10.0,
        timeout: float = 120.0,
    ):
        """
        Initialize LLM client.
        
        Args:
            provider: "openrouter" or "chutes"
            api_key: API key (or set via environment variable)
            cost_limit: Maximum cost in USD (default: $10)
            timeout: Request timeout in seconds
        """
        self.provider = provider.lower()
        if self.provider not in self.PROVIDERS:
            raise ValueError(f"Unknown provider: {provider}. Use 'openrouter' or 'chutes'")
        
        provider_config = self.PROVIDERS[self.provider]
        self.base_url = provider_config["url"]
        
        # Get API key from param or environment
        self.api_key = api_key or os.environ.get(provider_config["env_key"])
        if not self.api_key:
            raise ValueError(
                f"API key required. Set {provider_config['env_key']} environment variable "
                f"or pass api_key parameter."
            )
        
        self.timeout = timeout
        self._cost_tracker = get_cost_tracker()
        self._cost_tracker.set_limit(cost_limit)
        self._client: Optional[httpx.AsyncClient] = None
    
    async def _get_client(self) -> httpx.AsyncClient:
        """Get or create HTTP client"""
        if self._client is None:
            self._client = httpx.AsyncClient(
                timeout=self.timeout,
                headers={
                    "Authorization": f"Bearer {self.api_key}",
                    "Content-Type": "application/json",
                    "HTTP-Referer": "https://term-challenge.platform.network",
                    "X-Title": "Term Challenge Agent",
                },
            )
        return self._client
    
    async def close(self):
        """Close the HTTP client"""
        if self._client:
            await self._client.aclose()
            self._client = None
    
    async def chat(
        self,
        messages: List[Union[Message, Dict[str, str]]],
        model: str = "openai/gpt-4o-mini",
        temperature: float = 0.7,
        max_tokens: Optional[int] = None,
        stop: Optional[List[str]] = None,
        **kwargs,
    ) -> ChatResponse:
        """
        Send a chat completion request.
        
        Args:
            messages: List of messages (dicts or Message objects)
            model: Model identifier (e.g., "openai/gpt-4o-mini")
            temperature: Sampling temperature (0-2)
            max_tokens: Maximum tokens to generate
            stop: Stop sequences
            **kwargs: Additional parameters passed to the API
            
        Returns:
            ChatResponse with content and metadata
            
        Raises:
            CostLimitExceeded: If cost limit would be exceeded
            ProviderError: If API request fails
        """
        # Check cost limit before request
        if self._cost_tracker.remaining <= 0:
            raise CostLimitExceeded(
                f"Cost limit of ${self._cost_tracker.limit:.2f} exceeded. "
                f"Total spent: ${self._cost_tracker.total_cost:.4f}"
            )
        
        # Convert messages to dicts
        msg_dicts = []
        for msg in messages:
            if isinstance(msg, Message):
                msg_dicts.append(msg.to_dict())
            else:
                msg_dicts.append(msg)
        
        # Build request
        payload = {
            "model": model,
            "messages": msg_dicts,
            "temperature": temperature,
            **kwargs,
        }
        
        if max_tokens:
            payload["max_tokens"] = max_tokens
        if stop:
            payload["stop"] = stop
        
        # Make request
        client = await self._get_client()
        
        try:
            response = await client.post(self.base_url, json=payload)
            response.raise_for_status()
            data = response.json()
        except httpx.HTTPStatusError as e:
            raise ProviderError(
                f"{self.provider} API error: {e.response.status_code} - {e.response.text}"
            )
        except httpx.RequestError as e:
            raise ProviderError(f"{self.provider} request failed: {str(e)}")
        
        # Parse response
        choice = data["choices"][0]
        usage = data.get("usage", {})
        
        # Calculate cost
        input_tokens = usage.get("prompt_tokens", 0)
        output_tokens = usage.get("completion_tokens", 0)
        cost = self._calculate_cost(model, input_tokens, output_tokens)
        
        # Track cost
        self._cost_tracker.add(cost, model, input_tokens, output_tokens)
        
        return ChatResponse(
            content=choice["message"]["content"],
            model=data.get("model", model),
            finish_reason=choice.get("finish_reason", "stop"),
            usage=usage,
            cost=cost,
            provider=self.provider,
            raw=data,
        )
    
    def _calculate_cost(self, model: str, input_tokens: int, output_tokens: int) -> float:
        """Calculate cost for a request"""
        # Normalize model name
        model_key = model.lower()
        
        # Get pricing (fallback to gpt-4o-mini pricing if unknown)
        pricing = MODEL_PRICING.get(model_key, MODEL_PRICING.get("openai/gpt-4o-mini", (0.15, 0.6)))
        input_price, output_price = pricing
        
        # Price is per million tokens
        cost = (input_tokens * input_price / 1_000_000) + (output_tokens * output_price / 1_000_000)
        return cost
    
    @property
    def total_cost(self) -> float:
        """Get total cost incurred"""
        return self._cost_tracker.total_cost
    
    @property
    def remaining_budget(self) -> float:
        """Get remaining budget"""
        return self._cost_tracker.remaining
    
    async def __aenter__(self):
        return self
    
    async def __aexit__(self, *args):
        await self.close()


# Default global client instance
_default_client: Optional[LLMClient] = None


def _get_default_client() -> LLMClient:
    """Get or create default client"""
    global _default_client
    if _default_client is None:
        # Try to auto-detect provider from environment
        if os.environ.get("CHUTES_API_KEY"):
            _default_client = LLMClient(provider="chutes")
        else:
            _default_client = LLMClient(provider="openrouter")
    return _default_client


class LLMModule:
    """
    Module-level LLM interface for convenient access.
    
    Usage:
        from term_sdk import llm
        
        response = await llm.chat([{"role": "user", "content": "Hi"}])
    """
    
    async def chat(
        self,
        messages: List[Union[Message, Dict[str, str]]],
        model: str = "openai/gpt-4o-mini",
        **kwargs,
    ) -> ChatResponse:
        """Send chat completion request using default client"""
        client = _get_default_client()
        return await client.chat(messages, model, **kwargs)
    
    def configure(
        self,
        provider: str = "openrouter",
        api_key: Optional[str] = None,
        cost_limit: float = 10.0,
    ):
        """Configure the default LLM client"""
        global _default_client
        _default_client = LLMClient(
            provider=provider,
            api_key=api_key,
            cost_limit=cost_limit,
        )
    
    @property
    def total_cost(self) -> float:
        """Get total cost incurred"""
        return _get_default_client().total_cost
    
    @property
    def remaining_budget(self) -> float:
        """Get remaining budget"""
        return _get_default_client().remaining_budget


# Global llm instance
llm = LLMModule()
