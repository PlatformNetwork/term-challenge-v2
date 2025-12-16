"""
Cost tracking for LLM usage
"""

from dataclasses import dataclass, field
from datetime import datetime
from typing import Dict, List, Optional, Tuple

# Model pricing in USD per million tokens (input, output)
MODEL_PRICING: Dict[str, Tuple[float, float]] = {
    # OpenAI models
    "openai/gpt-4o": (2.50, 10.00),
    "openai/gpt-4o-mini": (0.15, 0.60),
    "openai/gpt-4-turbo": (10.00, 30.00),
    "openai/gpt-4": (30.00, 60.00),
    "openai/gpt-3.5-turbo": (0.50, 1.50),
    "openai/o1-preview": (15.00, 60.00),
    "openai/o1-mini": (3.00, 12.00),
    
    # Anthropic models
    "anthropic/claude-3.5-sonnet": (3.00, 15.00),
    "anthropic/claude-3-sonnet": (3.00, 15.00),
    "anthropic/claude-3-haiku": (0.25, 1.25),
    "anthropic/claude-3-opus": (15.00, 75.00),
    "anthropic/claude-2": (8.00, 24.00),
    
    # Meta models
    "meta-llama/llama-3.1-70b-instruct": (0.52, 0.75),
    "meta-llama/llama-3.1-8b-instruct": (0.055, 0.055),
    "meta-llama/llama-3-70b-instruct": (0.52, 0.75),
    
    # Mistral models
    "mistralai/mistral-large": (2.00, 6.00),
    "mistralai/mistral-medium": (2.70, 8.10),
    "mistralai/mistral-small": (0.20, 0.60),
    "mistralai/mixtral-8x7b-instruct": (0.24, 0.24),
    
    # Google models
    "google/gemini-pro": (0.125, 0.375),
    "google/gemini-pro-1.5": (1.25, 5.00),
    
    # Cohere models
    "cohere/command-r-plus": (3.00, 15.00),
    "cohere/command-r": (0.50, 1.50),
    
    # DeepSeek
    "deepseek/deepseek-chat": (0.14, 0.28),
    "deepseek/deepseek-coder": (0.14, 0.28),
}

# Aliases
MODEL_PRICING["gpt-4o"] = MODEL_PRICING["openai/gpt-4o"]
MODEL_PRICING["gpt-4o-mini"] = MODEL_PRICING["openai/gpt-4o-mini"]
MODEL_PRICING["claude-3.5-sonnet"] = MODEL_PRICING["anthropic/claude-3.5-sonnet"]
MODEL_PRICING["claude-3-haiku"] = MODEL_PRICING["anthropic/claude-3-haiku"]


@dataclass
class UsageRecord:
    """Record of a single LLM call"""
    timestamp: datetime
    model: str
    input_tokens: int
    output_tokens: int
    cost: float
    
    @property
    def total_tokens(self) -> int:
        return self.input_tokens + self.output_tokens


@dataclass
class CostTracker:
    """
    Tracks LLM usage and costs.
    
    Usage:
        tracker = CostTracker(limit=10.0)
        tracker.add(0.001, "gpt-4o-mini", 100, 50)
        print(f"Total: ${tracker.total_cost:.4f}")
        print(f"Remaining: ${tracker.remaining:.4f}")
    """
    
    limit: float = 10.0
    records: List[UsageRecord] = field(default_factory=list)
    
    @property
    def total_cost(self) -> float:
        """Total cost incurred"""
        return sum(r.cost for r in self.records)
    
    @property
    def remaining(self) -> float:
        """Remaining budget"""
        return max(0.0, self.limit - self.total_cost)
    
    @property
    def total_tokens(self) -> int:
        """Total tokens used"""
        return sum(r.total_tokens for r in self.records)
    
    @property
    def total_input_tokens(self) -> int:
        """Total input tokens used"""
        return sum(r.input_tokens for r in self.records)
    
    @property
    def total_output_tokens(self) -> int:
        """Total output tokens used"""
        return sum(r.output_tokens for r in self.records)
    
    def set_limit(self, limit: float):
        """Set the cost limit"""
        self.limit = limit
    
    def add(
        self,
        cost: float,
        model: str,
        input_tokens: int,
        output_tokens: int,
    ) -> UsageRecord:
        """
        Record a usage.
        
        Args:
            cost: Cost in USD
            model: Model name
            input_tokens: Number of input tokens
            output_tokens: Number of output tokens
            
        Returns:
            The created UsageRecord
        """
        record = UsageRecord(
            timestamp=datetime.now(),
            model=model,
            input_tokens=input_tokens,
            output_tokens=output_tokens,
            cost=cost,
        )
        self.records.append(record)
        return record
    
    def can_afford(self, estimated_cost: float) -> bool:
        """Check if an estimated cost can be afforded"""
        return self.remaining >= estimated_cost
    
    def estimate_cost(
        self,
        model: str,
        input_tokens: int,
        max_output_tokens: int = 1000,
    ) -> float:
        """
        Estimate cost for a request.
        
        Args:
            model: Model name
            input_tokens: Number of input tokens
            max_output_tokens: Maximum expected output tokens
            
        Returns:
            Estimated cost in USD
        """
        model_key = model.lower()
        pricing = MODEL_PRICING.get(model_key, (0.15, 0.6))
        input_price, output_price = pricing
        
        return (input_tokens * input_price / 1_000_000) + (max_output_tokens * output_price / 1_000_000)
    
    def get_summary(self) -> Dict:
        """Get usage summary"""
        model_costs: Dict[str, float] = {}
        model_tokens: Dict[str, int] = {}
        
        for record in self.records:
            model = record.model
            model_costs[model] = model_costs.get(model, 0) + record.cost
            model_tokens[model] = model_tokens.get(model, 0) + record.total_tokens
        
        return {
            "total_cost": self.total_cost,
            "remaining": self.remaining,
            "limit": self.limit,
            "total_tokens": self.total_tokens,
            "total_requests": len(self.records),
            "by_model": {
                model: {
                    "cost": model_costs[model],
                    "tokens": model_tokens[model],
                }
                for model in model_costs
            },
        }
    
    def reset(self):
        """Reset all records"""
        self.records.clear()


# Global cost tracker
_global_tracker: Optional[CostTracker] = None


def get_cost_tracker() -> CostTracker:
    """Get the global cost tracker"""
    global _global_tracker
    if _global_tracker is None:
        _global_tracker = CostTracker()
    return _global_tracker


def reset_cost_tracker():
    """Reset the global cost tracker"""
    global _global_tracker
    _global_tracker = CostTracker()
