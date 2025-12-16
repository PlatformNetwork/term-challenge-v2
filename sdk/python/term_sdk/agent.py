"""
Agent base class and registration for Term Challenge.

Compatible with terminal-bench protocol.
"""

import asyncio
import functools
from abc import ABC, abstractmethod
from typing import Any, Callable, Dict, List, Optional, Type, TypeVar, Union

from .cost import get_cost_tracker
from .exceptions import ValidationError
from .protocol import AgentResponse, Command, run, cmd, keys, wait, Keys

T = TypeVar("T")

_registered_agents: Dict[str, Type] = {}


class Agent(ABC):
    """
    Base class for Term Challenge agents.
    
    Compatible with terminal-bench protocol. Implement the `step()` method
    to create an agent that runs in a loop until task_complete is True.
    
    Example:
        from term_sdk import Agent, AgentResponse, run
        
        @Agent.register
        class MyAgent(Agent):
            async def step(self, task: str, terminal_state: str) -> AgentResponse:
                return AgentResponse(
                    analysis="I see the terminal prompt",
                    plan="Run ls to see files",
                    commands=[run("ls -la")],
                    task_complete=False
                )
    """
    
    name: str = "Agent"
    description: str = ""
    version: str = "0.1.0"
    
    def __init__(self, **kwargs):
        self._cost_tracker = get_cost_tracker()
        self._tools: Dict[str, Callable] = {}
        self._episode = 0
        self._max_episodes = kwargs.get("max_episodes", 50)
        self.config = kwargs
    
    @classmethod
    def register(cls, agent_cls: Type[T] = None, *, name: str = None) -> Type[T]:
        """
        Decorator to register an agent class.
        
        Usage:
            @Agent.register
            class MyAgent(Agent):
                async def step(self, task, terminal_state) -> AgentResponse:
                    ...
            
            # Or with custom name
            @Agent.register(name="my-agent")
            class MyAgent(Agent):
                ...
        """
        def decorator(agent_cls: Type[T]) -> Type[T]:
            agent_name = name or agent_cls.__name__
            _registered_agents[agent_name] = agent_cls
            agent_cls.name = agent_name
            return agent_cls
        
        if agent_cls is not None:
            return decorator(agent_cls)
        return decorator
    
    @classmethod
    def get(cls, name: str) -> Optional[Type["Agent"]]:
        """Get a registered agent by name"""
        return _registered_agents.get(name)
    
    @classmethod
    def list_agents(cls) -> List[str]:
        """List all registered agent names"""
        return list(_registered_agents.keys())
    
    async def setup(self) -> None:
        """Called once before the first step. Override to initialize resources."""
        pass
    
    async def teardown(self) -> None:
        """Called after task completion. Override to cleanup resources."""
        pass
    
    @abstractmethod
    async def step(self, task: str, terminal_state: str) -> AgentResponse:
        """
        Execute one step of the agent.
        
        This is the main method you must implement. It receives:
        - task: The task description/instruction
        - terminal_state: Current terminal screen content
        
        Returns:
            AgentResponse with analysis, plan, commands, and task_complete flag
        """
        raise NotImplementedError("Implement step() in your agent")
    
    async def solve(self, task: str, initial_state: str = "") -> AgentResponse:
        """
        Solve a task by running step() in a loop.
        
        This is called by the harness. You typically don't override this.
        """
        await self.setup()
        
        terminal_state = initial_state
        response = None
        
        try:
            for episode in range(self._max_episodes):
                self._episode = episode
                response = await self.step(task, terminal_state)
                
                if response.task_complete:
                    break
                
                # In real execution, harness updates terminal_state
                # For standalone testing, we just continue
            
            return response or AgentResponse(
                analysis="Max episodes reached",
                plan="",
                commands=[],
                task_complete=True
            )
        finally:
            await self.teardown()
    
    def get_cost(self) -> float:
        """Get the total cost incurred so far"""
        return self._cost_tracker.total_cost
    
    def get_remaining_budget(self) -> float:
        """Get remaining budget"""
        return self._cost_tracker.remaining
    
    @property
    def episode(self) -> int:
        """Current episode number"""
        return self._episode


class SimpleAgent(Agent):
    """
    Simplified agent that solves the task in one shot.
    
    For agents that don't need the step-by-step loop.
    Override solve_task() instead of step().
    
    Example:
        @Agent.register
        class MySimpleAgent(SimpleAgent):
            async def solve_task(self, task: str) -> str:
                # Do everything and return result
                return "solution"
    """
    
    @abstractmethod
    async def solve_task(self, task: str) -> str:
        """Solve the task in one shot. Override this."""
        raise NotImplementedError
    
    async def step(self, task: str, terminal_state: str) -> AgentResponse:
        """Wrap solve_task in a single step"""
        result = await self.solve_task(task)
        return AgentResponse(
            analysis=f"Task completed: {result[:100]}..." if len(result) > 100 else f"Task completed: {result}",
            plan="Return result",
            commands=[],
            task_complete=True
        )


class LLMAgent(Agent):
    """
    Agent that uses an LLM to decide actions.
    
    Provides built-in LLM integration with prompt management.
    
    Example:
        @Agent.register
        class MyLLMAgent(LLMAgent):
            system_prompt = "You are a terminal expert..."
            
            async def step(self, task: str, terminal_state: str) -> AgentResponse:
                response = await self.llm.chat(
                    messages=self.build_messages(task, terminal_state),
                    model="openai/gpt-4o-mini"
                )
                return self.parse_response(response.content)
    """
    
    system_prompt: str = """You are an AI agent solving terminal tasks.
Analyze the terminal state and provide commands to execute.
Respond in JSON format with: analysis, plan, commands, task_complete."""
    
    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self._llm = None
        self._messages: List[Dict[str, str]] = []
    
    @property
    def llm(self):
        """Get the LLM client (lazy initialization)"""
        if self._llm is None:
            from .llm import llm
            self._llm = llm
        return self._llm
    
    def build_messages(
        self,
        task: str,
        terminal_state: str,
        include_history: bool = True
    ) -> List[Dict[str, str]]:
        """Build messages for LLM"""
        messages = [{"role": "system", "content": self.system_prompt}]
        
        if include_history:
            messages.extend(self._messages)
        
        user_content = f"""Task: {task}

Current terminal state:
{terminal_state}

Respond with JSON: {{"analysis": "...", "plan": "...", "commands": [{{"keystrokes": "...", "duration": 0.1}}], "task_complete": false}}"""
        
        messages.append({"role": "user", "content": user_content})
        return messages
    
    def parse_response(self, content: str) -> AgentResponse:
        """Parse LLM response into AgentResponse"""
        import json
        import re
        
        # Try to extract JSON from response
        json_match = re.search(r'\{[\s\S]*\}', content)
        if json_match:
            try:
                data = json.loads(json_match.group())
                return AgentResponse.from_dict(data)
            except json.JSONDecodeError:
                pass
        
        # Fallback: treat entire response as analysis
        return AgentResponse(
            analysis=content,
            plan="Parse failed - trying to continue",
            commands=[],
            task_complete=False
        )


def agent(
    func: Callable = None,
    *,
    name: Optional[str] = None,
    description: Optional[str] = None,
) -> Callable:
    """
    Decorator to create an agent from a function.
    
    The function receives (task, terminal_state) and returns AgentResponse or dict.
    
    Example:
        @agent
        async def my_agent(task: str, terminal_state: str) -> AgentResponse:
            return AgentResponse(
                analysis="...",
                plan="...",
                commands=[run("ls")],
                task_complete=False
            )
        
        # Or return a dict
        @agent(name="simple")
        async def simple_agent(task, state):
            return {
                "analysis": "...",
                "plan": "...",
                "commands": [{"keystrokes": "ls\\n", "duration": 0.1}],
                "task_complete": False
            }
    """
    def decorator(func: Callable) -> Type[Agent]:
        agent_name = name or func.__name__
        
        class FunctionAgent(Agent):
            async def step(self, task: str, terminal_state: str) -> AgentResponse:
                if asyncio.iscoroutinefunction(func):
                    result = await func(task, terminal_state)
                else:
                    result = func(task, terminal_state)
                
                if isinstance(result, dict):
                    return AgentResponse.from_dict(result)
                return result
        
        FunctionAgent.__name__ = agent_name
        FunctionAgent.name = agent_name
        FunctionAgent.description = description or ""
        _registered_agents[agent_name] = FunctionAgent
        return FunctionAgent
    
    if func is not None:
        return decorator(func)
    return decorator


def get_registered_agents() -> Dict[str, Type]:
    """Get all registered agent classes"""
    return _registered_agents.copy()


async def run_agent(agent_instance: Agent, task: str, initial_state: str = "") -> AgentResponse:
    """
    Run an agent's solve method with proper lifecycle.
    
    Args:
        agent_instance: The agent instance
        task: The task to solve
        initial_state: Initial terminal state
        
    Returns:
        The final AgentResponse
    """
    return await agent_instance.solve(task, initial_state)
