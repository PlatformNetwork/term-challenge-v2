#!/usr/bin/env python3
"""
Simple Term Challenge Agent Example

This agent uses GPT-4o-mini to solve terminal tasks step by step.
"""

import asyncio
import os
from term_sdk import Agent, AgentResponse, Command, run, llm

# Configure LLM
llm.configure(
    provider="openrouter",
    api_key=os.environ.get("OPENROUTER_API_KEY"),
    cost_limit=5.0  # $5 budget
)


@Agent.register(name="simple-agent")
class SimpleAgent(Agent):
    """A simple agent that uses LLM to solve terminal tasks."""
    
    SYSTEM_PROMPT = """You are an expert Linux terminal user.
You receive a task and the current terminal state.
Respond with the NEXT SINGLE COMMAND to execute.
Just output the command, nothing else.
If the task is complete, respond with: DONE"""

    async def step(self, task: str, terminal_state: str) -> AgentResponse:
        # Build prompt
        user_message = f"""Task: {task}

Current terminal output:
```
{terminal_state[-2000:]}
```

What is the next command? (or DONE if finished)"""

        # Ask LLM
        response = await llm.chat(
            messages=[
                {"role": "system", "content": self.SYSTEM_PROMPT},
                {"role": "user", "content": user_message}
            ],
            model="openai/gpt-4o-mini",
            temperature=0.3
        )
        
        command = response.content.strip()
        
        # Check if task is complete
        if "DONE" in command.upper():
            return AgentResponse(
                analysis=f"Task appears complete. Terminal shows: {terminal_state[-200:]}",
                plan="Mark task as complete",
                commands=[],
                task_complete=True
            )
        
        # Execute the command
        return AgentResponse(
            analysis=f"LLM suggested: {command}",
            plan=f"Execute: {command}",
            commands=[run(command, duration=1.0)],
            task_complete=False
        )


async def main():
    """Test the agent standalone"""
    agent = SimpleAgent()
    
    # Simulate a task
    task = "Create a Python script that prints 'Hello World' and run it"
    initial_state = "user@sandbox:~$ "
    
    print(f"Task: {task}")
    print("-" * 50)
    
    # Run agent loop (simulated)
    terminal_state = initial_state
    for i in range(10):
        response = await agent.step(task, terminal_state)
        
        print(f"\nStep {i + 1}:")
        print(f"  Analysis: {response.analysis[:80]}...")
        print(f"  Plan: {response.plan}")
        print(f"  Commands: {[c.keystrokes for c in response.commands]}")
        print(f"  Complete: {response.task_complete}")
        
        if response.task_complete:
            print("\n✓ Task completed!")
            break
        
        # Simulate terminal output (in real harness, this comes from tmux)
        for cmd in response.commands:
            terminal_state += f"\n{cmd.keystrokes.strip()}\n[simulated output]"
    
    print(f"\nTotal LLM cost: ${llm.total_cost:.4f}")
    print(f"Remaining budget: ${llm.remaining_budget:.2f}")


if __name__ == "__main__":
    asyncio.run(main())
