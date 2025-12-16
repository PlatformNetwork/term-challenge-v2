"""
Term Challenge Harness - Professional agent runner framework.

This module provides the core infrastructure for running agents that communicate
with the Term Challenge harness via stdin/stdout JSON protocol.

Example:
    ```python
    from term_sdk import Agent, AgentResponse, Command
    from term_sdk.harness import Harness
    
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
"""

from __future__ import annotations

import sys
import json
import asyncio
import logging
import traceback
from abc import ABC, abstractmethod
from dataclasses import dataclass, field, asdict
from typing import List, Optional, Dict, Any, Callable, TypeVar, Union
from enum import Enum

# Configure logging to stderr (stdout is reserved for protocol)
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s [%(levelname)s] %(name)s: %(message)s',
    stream=sys.stderr
)
logger = logging.getLogger("term_sdk")


# =============================================================================
# Protocol Types
# =============================================================================

@dataclass
class Command:
    """A command to send to the terminal.
    
    Attributes:
        keystrokes: The exact text to send (include \\n to execute).
        duration: Seconds to wait after sending (default 1.0).
    """
    keystrokes: str
    duration: float = 1.0
    
    def to_dict(self) -> Dict[str, Any]:
        return {"keystrokes": self.keystrokes, "duration": self.duration}


@dataclass
class AgentRequest:
    """Request from harness to agent.
    
    Attributes:
        instruction: The task instruction/goal.
        screen: Current terminal screen content.
        step: Current step number (1-indexed).
    """
    instruction: str
    screen: str
    step: int
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "AgentRequest":
        return cls(
            instruction=data.get("instruction", ""),
            screen=data.get("screen", ""),
            step=data.get("step", 1)
        )


@dataclass
class AgentResponse:
    """Response from agent to harness.
    
    Attributes:
        analysis: Your analysis of the current terminal state.
        plan: Your plan for the next steps.
        commands: List of commands to execute.
        task_complete: Set True when task is finished.
    """
    analysis: str = ""
    plan: str = ""
    commands: List[Command] = field(default_factory=list)
    task_complete: bool = False
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "analysis": self.analysis,
            "plan": self.plan,
            "commands": [c.to_dict() if isinstance(c, Command) else c for c in self.commands],
            "task_complete": self.task_complete
        }
    
    @classmethod
    def error(cls, message: str) -> "AgentResponse":
        """Create an error response."""
        return cls(
            analysis=f"Error: {message}",
            plan="Cannot continue due to error",
            commands=[],
            task_complete=False
        )


# =============================================================================
# Base Agent
# =============================================================================

class Agent(ABC):
    """Base class for Term Challenge agents.
    
    Subclass this and implement the `step` method to create your agent.
    
    Example:
        ```python
        class MyAgent(Agent):
            async def setup(self):
                # Initialize resources (optional)
                self.client = SomeLLMClient()
            
            async def step(self, instruction: str, screen: str, step: int) -> AgentResponse:
                # Your agent logic here
                response = await self.client.chat(...)
                return AgentResponse(
                    analysis="...",
                    plan="...",
                    commands=[Command("ls\\n")],
                    task_complete=False
                )
            
            async def cleanup(self):
                # Clean up resources (optional)
                await self.client.close()
        ```
    """
    
    async def setup(self) -> None:
        """Initialize the agent. Override to set up resources."""
        pass
    
    @abstractmethod
    async def step(self, instruction: str, screen: str, step: int) -> AgentResponse:
        """Process one step of the task.
        
        Args:
            instruction: The task instruction/goal.
            screen: Current terminal screen content.
            step: Current step number (1-indexed).
        
        Returns:
            AgentResponse with analysis, plan, commands, and task_complete flag.
        """
        raise NotImplementedError
    
    async def cleanup(self) -> None:
        """Clean up resources. Override to release resources."""
        pass


# =============================================================================
# Harness
# =============================================================================

class Harness:
    """Runs an agent in the Term Challenge harness.
    
    The harness handles:
    - Reading requests from stdin
    - Calling the agent's step method
    - Writing responses to stdout
    - Error handling and logging
    
    Example:
        ```python
        agent = MyAgent()
        harness = Harness(agent)
        harness.run()
        ```
    """
    
    def __init__(self, agent: Agent):
        """Initialize the harness with an agent.
        
        Args:
            agent: The agent instance to run.
        """
        self.agent = agent
        self._running = False
    
    def run(self) -> None:
        """Run the agent loop (blocking).
        
        This is the main entry point. Call this from your script's main block.
        """
        try:
            asyncio.run(self._run_async())
        except KeyboardInterrupt:
            logger.info("Agent interrupted by user")
        except Exception as e:
            logger.error(f"Fatal error: {e}")
            traceback.print_exc(file=sys.stderr)
            sys.exit(1)
    
    async def _run_async(self) -> None:
        """Async implementation of the agent loop."""
        self._running = True
        
        # Setup
        try:
            logger.info("Setting up agent...")
            await self.agent.setup()
            logger.info("Agent ready")
        except Exception as e:
            logger.error(f"Setup failed: {e}")
            self._send_response(AgentResponse.error(f"Setup failed: {e}"))
            return
        
        try:
            # Main loop
            await self._process_loop()
        finally:
            # Cleanup
            try:
                await self.agent.cleanup()
            except Exception as e:
                logger.error(f"Cleanup error: {e}")
    
    async def _process_loop(self) -> None:
        """Process requests from stdin."""
        for line in sys.stdin:
            if not self._running:
                break
            
            line = line.strip()
            if not line:
                continue
            
            try:
                response = await self._process_request(line)
                self._send_response(response)
            except Exception as e:
                logger.error(f"Error processing request: {e}")
                traceback.print_exc(file=sys.stderr)
                self._send_response(AgentResponse.error(str(e)))
    
    async def _process_request(self, line: str) -> AgentResponse:
        """Process a single request line."""
        # Parse request
        try:
            data = json.loads(line)
            request = AgentRequest.from_dict(data)
        except json.JSONDecodeError as e:
            logger.error(f"Invalid JSON: {e}")
            return AgentResponse.error(f"Invalid JSON: {e}")
        
        logger.debug(f"Step {request.step}: Processing...")
        
        # Call agent
        response = await self.agent.step(
            request.instruction,
            request.screen,
            request.step
        )
        
        logger.debug(f"Step {request.step}: Complete (task_complete={response.task_complete})")
        return response
    
    def _send_response(self, response: AgentResponse) -> None:
        """Send response to stdout."""
        try:
            data = response.to_dict()
            print(json.dumps(data), flush=True)
        except Exception as e:
            logger.error(f"Failed to send response: {e}")
            # Send minimal error response
            print(json.dumps({
                "analysis": f"Error: {e}",
                "plan": "",
                "commands": [],
                "task_complete": False
            }), flush=True)
    
    def stop(self) -> None:
        """Stop the agent loop."""
        self._running = False


# =============================================================================
# Convenience Functions
# =============================================================================

def run(agent: Agent) -> None:
    """Run an agent in the harness.
    
    This is a convenience function equivalent to:
        Harness(agent).run()
    
    Args:
        agent: The agent instance to run.
    """
    Harness(agent).run()


# Legacy compatibility
def run_agent_loop(agent: Agent) -> None:
    """Run an agent (legacy function name).
    
    Deprecated: Use `run(agent)` or `Harness(agent).run()` instead.
    """
    run(agent)


# =============================================================================
# Exports
# =============================================================================

__all__ = [
    "Command",
    "AgentRequest", 
    "AgentResponse",
    "Agent",
    "Harness",
    "run",
    "run_agent_loop",
]
