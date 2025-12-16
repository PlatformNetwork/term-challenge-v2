#!/usr/bin/env python3
"""
Advanced Term Challenge Agent Example

Features:
- Multi-step planning with memory
- Error recovery
- Tool use (file operations, etc.)
- Cost-aware execution
"""

import asyncio
import json
import os
import re
from typing import List, Dict, Optional
from dataclasses import dataclass, field

from term_sdk import (
    Agent, AgentResponse, Command, run, cmd, keys, wait,
    llm, terminal, Keys
)


@dataclass
class Memory:
    """Agent memory for tracking state"""
    task: str = ""
    plan: List[str] = field(default_factory=list)
    completed_steps: List[str] = field(default_factory=list)
    errors: List[str] = field(default_factory=list)
    files_created: List[str] = field(default_factory=list)
    current_directory: str = "~"


@Agent.register(name="advanced-agent")
class AdvancedAgent(Agent):
    """
    Advanced agent with planning, memory, and error recovery.
    """
    
    PLANNER_PROMPT = """You are a planning AI for terminal tasks.
Given a task, create a step-by-step plan.
Output a JSON array of steps, each step being a string description.

Example:
["List current directory", "Create file hello.py", "Write Python code", "Run the script", "Verify output"]

Task: {task}

Output ONLY the JSON array, nothing else."""

    EXECUTOR_PROMPT = """You are an expert Linux terminal user.
You have a plan and need to execute the next step.

Task: {task}
Plan: {plan}
Completed: {completed}
Current step: {current_step}
Terminal state:
```
{terminal_state}
```

Output the exact command(s) to execute for this step.
If multiple commands, separate with newlines.
If the step is already done based on terminal output, output: SKIP
If all steps are done, output: DONE

Commands:"""

    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self.memory = Memory()
        self.planning_done = False
    
    async def setup(self):
        """Initialize agent"""
        # Configure LLM with cost tracking
        llm.configure(
            provider="openrouter",
            api_key=os.environ.get("OPENROUTER_API_KEY"),
            cost_limit=10.0
        )
    
    async def _create_plan(self, task: str) -> List[str]:
        """Create a plan for the task"""
        response = await llm.chat(
            messages=[{
                "role": "user",
                "content": self.PLANNER_PROMPT.format(task=task)
            }],
            model="openai/gpt-4o-mini",
            temperature=0.3
        )
        
        # Parse JSON plan
        try:
            # Find JSON array in response
            match = re.search(r'\[.*\]', response.content, re.DOTALL)
            if match:
                return json.loads(match.group())
        except json.JSONDecodeError:
            pass
        
        # Fallback: split by lines
        return [line.strip() for line in response.content.split('\n') if line.strip()]
    
    async def _get_next_commands(
        self,
        task: str,
        current_step: str,
        terminal_state: str
    ) -> List[str]:
        """Get commands for the current step"""
        response = await llm.chat(
            messages=[{
                "role": "user",
                "content": self.EXECUTOR_PROMPT.format(
                    task=task,
                    plan=json.dumps(self.memory.plan),
                    completed=json.dumps(self.memory.completed_steps),
                    current_step=current_step,
                    terminal_state=terminal_state[-3000:]
                )
            }],
            model="openai/gpt-4o-mini",
            temperature=0.2
        )
        
        content = response.content.strip()
        
        if "DONE" in content.upper():
            return ["DONE"]
        if "SKIP" in content.upper():
            return ["SKIP"]
        
        # Parse commands
        commands = []
        for line in content.split('\n'):
            line = line.strip()
            if line and not line.startswith('#'):
                # Remove common prefixes
                line = re.sub(r'^[$>]\s*', '', line)
                line = re.sub(r'^\d+\.\s*', '', line)
                if line:
                    commands.append(line)
        
        return commands if commands else [content]
    
    async def step(self, task: str, terminal_state: str) -> AgentResponse:
        """Execute one step of the agent"""
        
        # First step: create plan
        if not self.planning_done:
            self.memory.task = task
            self.memory.plan = await self._create_plan(task)
            self.planning_done = True
            
            return AgentResponse(
                analysis=f"Created plan with {len(self.memory.plan)} steps",
                plan=json.dumps(self.memory.plan, indent=2),
                commands=[run("pwd"), run("ls -la")],  # Orient ourselves
                task_complete=False
            )
        
        # Check remaining budget
        if llm.remaining_budget < 0.10:
            return AgentResponse(
                analysis="Budget nearly exhausted",
                plan="Complete task with remaining budget",
                commands=[],
                task_complete=True
            )
        
        # Get current step
        step_idx = len(self.memory.completed_steps)
        if step_idx >= len(self.memory.plan):
            return AgentResponse(
                analysis="All planned steps completed",
                plan="Verify and complete",
                commands=[],
                task_complete=True
            )
        
        current_step = self.memory.plan[step_idx]
        
        # Get commands for this step
        commands_str = await self._get_next_commands(task, current_step, terminal_state)
        
        # Handle special responses
        if commands_str[0] == "DONE":
            return AgentResponse(
                analysis="Task completed successfully",
                plan="All steps done",
                commands=[],
                task_complete=True
            )
        
        if commands_str[0] == "SKIP":
            self.memory.completed_steps.append(f"{current_step} (skipped)")
            return AgentResponse(
                analysis=f"Step already done: {current_step}",
                plan="Move to next step",
                commands=[],
                task_complete=False
            )
        
        # Build commands
        commands = []
        for cmd_str in commands_str[:3]:  # Limit to 3 commands per step
            # Detect file creation
            if 'echo' in cmd_str and '>' in cmd_str:
                match = re.search(r'>\s*(\S+)', cmd_str)
                if match:
                    self.memory.files_created.append(match.group(1))
            
            commands.append(run(cmd_str, duration=2.0))
        
        self.memory.completed_steps.append(current_step)
        
        return AgentResponse(
            analysis=f"Executing step {step_idx + 1}/{len(self.memory.plan)}: {current_step}",
            plan=f"Commands: {commands_str}",
            commands=commands,
            task_complete=False
        )
    
    async def teardown(self):
        """Cleanup and report"""
        print(f"\n=== Agent Report ===")
        print(f"Task: {self.memory.task}")
        print(f"Steps completed: {len(self.memory.completed_steps)}/{len(self.memory.plan)}")
        print(f"Files created: {self.memory.files_created}")
        print(f"Total cost: ${llm.total_cost:.4f}")
        print(f"Remaining budget: ${llm.remaining_budget:.2f}")


async def main():
    """Test the advanced agent"""
    agent = AdvancedAgent()
    await agent.setup()
    
    task = """Create a Python web scraper that:
1. Fetches https://example.com
2. Extracts the title and all links
3. Saves results to results.json
4. Prints a summary"""
    
    print(f"Task: {task}")
    print("=" * 60)
    
    terminal_state = "user@sandbox:~$ "
    
    for i in range(20):
        response = await agent.step(task, terminal_state)
        
        print(f"\n--- Step {i + 1} ---")
        print(f"Analysis: {response.analysis}")
        print(f"Plan: {response.plan[:100]}...")
        print(f"Commands: {[c.keystrokes.strip() for c in response.commands]}")
        
        if response.task_complete:
            print("\n✓ Task completed!")
            break
        
        # Simulate terminal
        for cmd in response.commands:
            terminal_state += f"\n$ {cmd.keystrokes.strip()}\n[output...]\n"
    
    await agent.teardown()


if __name__ == "__main__":
    asyncio.run(main())
