#!/usr/bin/env python3
"""
LLM Agent - Professional terminal agent powered by LLM.

This agent uses an LLM to analyze terminal state and execute commands
to complete tasks. Supports OpenRouter and Chutes providers.

Usage:
    # Set API key
    export OPENROUTER_API_KEY="sk-or-..."
    
    # Run with term CLI
    term bench agent -a ./llm_agent.py -t ~/.cache/term-challenge/datasets/hello-world
    
    # Or run standalone (for testing)
    python llm_agent.py
"""

from __future__ import annotations

import os
import sys
import json
import logging
from typing import List, Dict, Any, Optional

# Add SDK to path if running standalone
SDK_PATH = os.path.join(os.path.dirname(__file__), '..', '..', 'python')
if SDK_PATH not in sys.path:
    sys.path.insert(0, SDK_PATH)

from term_sdk.harness import Agent, AgentResponse, Command, Harness
from term_sdk.llm_client import LLMClient, Provider, ChatResponse

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s [%(levelname)s] %(message)s',
    stream=sys.stderr
)
logger = logging.getLogger(__name__)


# =============================================================================
# System Prompt
# =============================================================================

SYSTEM_PROMPT = """You are an expert terminal agent. Your goal is to complete tasks using only terminal commands.

## Response Format

Respond with JSON only:
```json
{
  "analysis": "What you observe in the terminal",
  "plan": "Your step-by-step plan",
  "commands": [
    {"keystrokes": "your_command\\n", "duration": 1.0}
  ],
  "task_complete": false
}
```

## Rules

1. **Commands**: Include `\\n` at the end to execute commands
2. **Duration**: Use longer durations (5-30s) for slow operations (apt, pip, npm)
3. **Verification**: Always verify your work before setting task_complete=true
4. **One step at a time**: Execute one logical operation per response
5. **Error handling**: If a command fails, analyze the error and try alternatives

## Special Keys

- `\\n` = Enter (execute command)
- `\\t` = Tab (autocomplete)
- `\\x03` = Ctrl+C (interrupt)
- `\\x04` = Ctrl+D (EOF)
- `\\x1b` = Escape

## Common Patterns

- Create file: `echo 'content' > file.txt\\n`
- Append to file: `echo 'content' >> file.txt\\n`
- View file: `cat file.txt\\n`
- List files: `ls -la\\n`
- Change directory: `cd /path\\n`
- Check current directory: `pwd\\n`

Remember: Only set task_complete=true after verifying the task is done."""


# =============================================================================
# LLM Agent
# =============================================================================

class LLMAgent(Agent):
    """Terminal agent powered by LLM.
    
    This agent:
    1. Receives terminal state from the harness
    2. Sends the state to an LLM with a specialized prompt
    3. Parses the LLM response into commands
    4. Returns commands to the harness for execution
    """
    
    def __init__(
        self,
        provider: str = None,
        model: str = None,
        api_key: str = None,
        budget: float = 10.0,
    ):
        """Initialize the agent.
        
        Args:
            provider: LLM provider (openrouter, chutes). Auto-detected if not set.
            model: Model name. Uses provider default if not set.
            api_key: API key. Uses environment variable if not set.
            budget: Maximum cost budget in USD.
        """
        self.provider = provider or os.environ.get("LLM_PROVIDER", "openrouter")
        self.model = model or os.environ.get("LLM_MODEL")
        self.api_key = api_key
        self.budget = budget
        self.client: Optional[LLMClient] = None
        self.conversation_history: List[Dict[str, str]] = []
    
    async def setup(self) -> None:
        """Initialize LLM client."""
        # Detect API key
        if not self.api_key:
            if self.provider == "openrouter":
                self.api_key = os.environ.get("OPENROUTER_API_KEY")
            elif self.provider == "chutes":
                self.api_key = os.environ.get("CHUTES_API_KEY")
            else:
                self.api_key = os.environ.get("LLM_API_KEY")
        
        if not self.api_key:
            raise ValueError(f"No API key found for provider '{self.provider}'")
        
        # Create client
        self.client = LLMClient(
            provider=self.provider,
            api_key=self.api_key,
            model=self.model,
            budget=self.budget,
        )
        
        logger.info(f"LLM Agent initialized: {self.provider}/{self.client.model}")
    
    async def step(self, instruction: str, screen: str, step: int) -> AgentResponse:
        """Process one step of the task.
        
        Args:
            instruction: Task instruction.
            screen: Current terminal screen.
            step: Step number.
        
        Returns:
            AgentResponse with commands to execute.
        """
        # Build user message
        user_message = self._build_prompt(instruction, screen, step)
        
        # Build messages
        messages = [
            {"role": "system", "content": SYSTEM_PROMPT},
            *self.conversation_history,
            {"role": "user", "content": user_message},
        ]
        
        # Call LLM
        try:
            response = await self.client.chat(
                messages=messages,
                temperature=0.7,
                max_tokens=2048,
            )
        except Exception as e:
            logger.error(f"LLM error: {e}")
            return AgentResponse.error(f"LLM error: {e}")
        
        # Log cost
        logger.info(
            f"Step {step}: {response.tokens} tokens, "
            f"${response.cost:.4f} (total: ${self.client.cost_tracker.total_cost:.4f})"
        )
        
        # Parse response
        agent_response = self._parse_response(response.content)
        
        # Update conversation history (keep last 10 exchanges)
        self.conversation_history.append({"role": "user", "content": user_message})
        self.conversation_history.append({"role": "assistant", "content": response.content})
        if len(self.conversation_history) > 20:
            self.conversation_history = self.conversation_history[-20:]
        
        return agent_response
    
    async def cleanup(self) -> None:
        """Clean up resources."""
        if self.client:
            await self.client.close()
            logger.info(
                f"Session complete: {self.client.cost_tracker.request_count} requests, "
                f"${self.client.cost_tracker.total_cost:.4f} total"
            )
    
    def _build_prompt(self, instruction: str, screen: str, step: int) -> str:
        """Build the user prompt."""
        return f"""## Task
{instruction}

## Terminal (Step {step})
```
{screen}
```

Analyze the terminal and respond with JSON for your next action."""
    
    def _parse_response(self, content: str) -> AgentResponse:
        """Parse LLM response into AgentResponse."""
        try:
            # Find JSON in response
            start = content.find('{')
            end = content.rfind('}')
            
            if start < 0 or end <= start:
                raise ValueError("No JSON found in response")
            
            json_str = content[start:end + 1]
            data = json.loads(json_str)
            
            # Parse commands
            commands = []
            for cmd in data.get("commands", []):
                if isinstance(cmd, dict):
                    commands.append(Command(
                        keystrokes=cmd.get("keystrokes", ""),
                        duration=float(cmd.get("duration", 1.0))
                    ))
                elif isinstance(cmd, str):
                    commands.append(Command(keystrokes=cmd))
            
            return AgentResponse(
                analysis=data.get("analysis", ""),
                plan=data.get("plan", ""),
                commands=commands,
                task_complete=bool(data.get("task_complete", False))
            )
            
        except (json.JSONDecodeError, ValueError) as e:
            logger.warning(f"Failed to parse response: {e}")
            logger.debug(f"Raw content: {content[:500]}")
            
            # Return error response
            return AgentResponse(
                analysis=f"Failed to parse LLM response: {e}",
                plan=content[:500] if content else "No content",
                commands=[],
                task_complete=False
            )


# =============================================================================
# Main
# =============================================================================

def main():
    """Main entry point."""
    # Create agent
    agent = LLMAgent(
        provider=os.environ.get("LLM_PROVIDER", "openrouter"),
        model=os.environ.get("LLM_MODEL"),
        budget=float(os.environ.get("LLM_BUDGET", "10.0")),
    )
    
    # Run in harness
    Harness(agent).run()


if __name__ == "__main__":
    main()
