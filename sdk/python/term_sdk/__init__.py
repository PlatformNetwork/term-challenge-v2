"""
Term Challenge SDK - Professional framework for building terminal agents.

Example:
    ```python
    from term_sdk import Agent, AgentResponse, Command, Harness
    
    class MyAgent(Agent):
        async def step(self, instruction: str, screen: str, step: int) -> AgentResponse:
            return AgentResponse(
                analysis="Terminal shows prompt",
                plan="Execute ls command",
                commands=[Command("ls -la\\n")],
                task_complete=False
            )
    
    if __name__ == "__main__":
        Harness(MyAgent()).run()
    ```

For LLM-powered agents:
    ```python
    from term_sdk import Agent, AgentResponse, Command, Harness
    from term_sdk.llm_client import LLMClient
    
    class LLMAgent(Agent):
        async def setup(self):
            self.client = LLMClient(provider="openrouter")
        
        async def step(self, instruction: str, screen: str, step: int) -> AgentResponse:
            response = await self.client.chat([
                {"role": "user", "content": f"Task: {instruction}\\n{screen}"}
            ])
            # Parse response and return AgentResponse
            ...
    ```
"""

__version__ = "0.2.0"

# Core harness types
from .harness import (
    Agent,
    AgentRequest,
    AgentResponse,
    Command,
    Harness,
    run,
    run_agent_loop,  # Legacy alias
)

# LLM client
from .llm_client import (
    LLMClient,
    Provider,
    Message,
    ChatResponse,
    CostTracker,
    MODEL_PRICING,
    estimate_cost,
    get_client,
    set_client,
    chat,
)

__all__ = [
    # Version
    "__version__",
    # Harness
    "Agent",
    "AgentRequest",
    "AgentResponse", 
    "Command",
    "Harness",
    "run",
    "run_agent_loop",
    # LLM
    "LLMClient",
    "Provider",
    "Message",
    "ChatResponse",
    "CostTracker",
    "MODEL_PRICING",
    "estimate_cost",
    "get_client",
    "set_client",
    "chat",
]
