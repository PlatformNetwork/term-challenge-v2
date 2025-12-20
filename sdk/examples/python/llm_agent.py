#!/usr/bin/env python3
"""
LLM-powered agent for Terminal Benchmark Challenge.

This agent uses an LLM to solve terminal tasks. It's designed to work
with the per-step execution model where each step is a fresh process.

The agent is stateless - it doesn't need to remember previous steps
because all context is provided in each request (instruction, output, etc.)

Usage:
    term bench agent -a llm_agent.py -t <task> -p openrouter --api-key "sk-..."
    term bench benchmark terminal-bench@2.0 -a llm_agent.py -p openrouter --api-key "sk-..."
"""
import os
import sys

from term_sdk import Agent, Request, Response, LLM, run, log


SYSTEM_PROMPT = """You are a terminal expert. You solve tasks by executing shell commands.

IMPORTANT RULES:
1. Execute ONE command at a time
2. After each command, analyze the output and decide next action
3. When the task is complete, respond with task_complete=true
4. Keep commands simple and portable (bash on Linux)

Output format (JSON):
- To run a command: {"command": "your command here", "task_complete": false}
- When done: {"command": null, "task_complete": true}

ONLY output valid JSON, nothing else."""


class LLMAgent(Agent):
    """
    Stateless LLM agent that solves terminal tasks.
    
    Each step receives full context via the Request object, so no
    state persistence is needed between steps.
    """
    
    def setup(self):
        """Initialize LLM client."""
        # Get model from environment or use default
        self.model = os.environ.get("LLM_MODEL", "anthropic/claude-sonnet-4")
        self.llm = LLM(default_model=self.model)
        log(f"Using model: {self.model}")
    
    def solve(self, req: Request) -> Response:
        """Process one step using LLM."""
        
        # Build prompt with full context
        prompt = self._build_prompt(req)
        
        # Get LLM response
        try:
            result = self.llm.ask(prompt, temperature=0.1)
            log(f"LLM response: {result.text[:100]}...")
            
            # Parse and return response
            return Response.from_llm(result.text)
            
        except Exception as e:
            log(f"LLM error: {e}")
            # On error, try to complete the task
            return Response.done()
    
    def _build_prompt(self, req: Request) -> str:
        """Build prompt with full context."""
        parts = [
            f"TASK: {req.instruction}",
            f"\nCURRENT STEP: {req.step}",
            f"WORKING DIRECTORY: {req.cwd}",
        ]
        
        if req.last_command:
            parts.append(f"\nLAST COMMAND: {req.last_command}")
            parts.append(f"EXIT CODE: {req.exit_code}")
            
            if req.output:
                # Truncate long output
                output = req.output[-3000:] if len(req.output) > 3000 else req.output
                parts.append(f"OUTPUT:\n```\n{output}\n```")
        
        if req.failed:
            parts.append("\nWARNING: Last command failed! Try a different approach.")
        
        parts.append("\nWhat single command should I run next? (JSON only)")
        
        return "\n".join(parts)
    
    def cleanup(self):
        """Report final stats."""
        stats = self.llm.get_stats()
        log(f"Total cost: ${stats['total_cost']:.4f}")
        log(f"Total tokens: {stats['total_tokens']}")


if __name__ == "__main__":
    run(LLMAgent())
