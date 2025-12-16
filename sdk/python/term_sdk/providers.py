"""
LLM Provider implementations
"""

import os
from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Any, Dict, List, Optional

import httpx

from .exceptions import ProviderError


@dataclass
class ProviderConfig:
    """Provider configuration"""
    name: str
    base_url: str
    api_key: str
    headers: Dict[str, str]


class Provider(ABC):
    """Base class for LLM providers"""
    
    name: str = "provider"
    
    def __init__(self, api_key: Optional[str] = None):
        self.api_key = api_key
        self._client: Optional[httpx.AsyncClient] = None
    
    @abstractmethod
    def get_config(self) -> ProviderConfig:
        """Get provider configuration"""
        pass
    
    async def _get_client(self) -> httpx.AsyncClient:
        """Get or create HTTP client"""
        if self._client is None:
            config = self.get_config()
            self._client = httpx.AsyncClient(
                timeout=120.0,
                headers={
                    "Authorization": f"Bearer {config.api_key}",
                    "Content-Type": "application/json",
                    **config.headers,
                },
            )
        return self._client
    
    async def close(self):
        """Close HTTP client"""
        if self._client:
            await self._client.aclose()
            self._client = None
    
    async def chat(
        self,
        messages: List[Dict[str, str]],
        model: str,
        **kwargs,
    ) -> Dict[str, Any]:
        """
        Send chat completion request.
        
        Args:
            messages: List of message dicts
            model: Model identifier
            **kwargs: Additional parameters
            
        Returns:
            Raw API response
        """
        config = self.get_config()
        client = await self._get_client()
        
        payload = {
            "model": model,
            "messages": messages,
            **kwargs,
        }
        
        try:
            response = await client.post(config.base_url, json=payload)
            response.raise_for_status()
            return response.json()
        except httpx.HTTPStatusError as e:
            raise ProviderError(
                f"{self.name} API error: {e.response.status_code} - {e.response.text}"
            )
        except httpx.RequestError as e:
            raise ProviderError(f"{self.name} request failed: {str(e)}")


class OpenRouterProvider(Provider):
    """
    OpenRouter provider - Access 100+ models through one API.
    
    https://openrouter.ai
    
    Usage:
        provider = OpenRouterProvider(api_key="your-key")
        # or set OPENROUTER_API_KEY environment variable
        
        response = await provider.chat(
            messages=[{"role": "user", "content": "Hello"}],
            model="openai/gpt-4o-mini"
        )
    """
    
    name = "openrouter"
    
    def __init__(self, api_key: Optional[str] = None):
        super().__init__(api_key or os.environ.get("OPENROUTER_API_KEY"))
        if not self.api_key:
            raise ValueError(
                "OpenRouter API key required. Set OPENROUTER_API_KEY environment variable "
                "or pass api_key parameter."
            )
    
    def get_config(self) -> ProviderConfig:
        return ProviderConfig(
            name="openrouter",
            base_url="https://openrouter.ai/api/v1/chat/completions",
            api_key=self.api_key,
            headers={
                "HTTP-Referer": "https://term-challenge.platform.network",
                "X-Title": "Term Challenge Agent",
            },
        )


class ChutesProvider(Provider):
    """
    Chutes provider - Fast, cheap LLM inference.
    
    https://llm.chutes.ai
    
    Usage:
        provider = ChutesProvider(api_key="your-key")
        # or set CHUTES_API_KEY environment variable
        
        response = await provider.chat(
            messages=[{"role": "user", "content": "Hello"}],
            model="gpt-4o-mini"
        )
    """
    
    name = "chutes"
    
    def __init__(self, api_key: Optional[str] = None):
        super().__init__(api_key or os.environ.get("CHUTES_API_KEY"))
        if not self.api_key:
            raise ValueError(
                "Chutes API key required. Set CHUTES_API_KEY environment variable "
                "or pass api_key parameter."
            )
    
    def get_config(self) -> ProviderConfig:
        return ProviderConfig(
            name="chutes",
            base_url="https://llm.chutes.ai/v1/chat/completions",
            api_key=self.api_key,
            headers={},
        )


def get_provider(name: str, api_key: Optional[str] = None) -> Provider:
    """
    Get a provider by name.
    
    Args:
        name: Provider name ("openrouter" or "chutes")
        api_key: Optional API key
        
    Returns:
        Provider instance
    """
    providers = {
        "openrouter": OpenRouterProvider,
        "chutes": ChutesProvider,
    }
    
    if name.lower() not in providers:
        raise ValueError(f"Unknown provider: {name}. Available: {list(providers.keys())}")
    
    return providers[name.lower()](api_key)
