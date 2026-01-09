#!/usr/bin/env python3
"""
Simple test agent for validating SDK 2.0 and compilation pipeline.
This agent performs basic operations to verify the agent framework works.
"""

from term_sdk import Agent, AgentContext, run


class TestAgent(Agent):
    """Minimal test agent for compilation verification."""
    
    def __init__(self):
        super().__init__()
        self.command_count = 0
    
    def run(self, ctx: AgentContext):
        """Execute a simple sequence of commands."""
        ctx.log(f"Task: {ctx.instruction[:50]}...")
        
        # Run a few test commands
        commands = [
            "echo 'Test agent started'",
            "ls -la",
            "pwd",
            "echo 'Test agent finished'",
        ]
        
        for cmd in commands:
            self.command_count += 1
            ctx.log(f"Command {self.command_count}: {cmd}")
            result = ctx.shell(cmd)
            if result.failed:
                ctx.log(f"Command failed: {result.stderr}")
        
        ctx.log(f"Executed {self.command_count} commands")
        ctx.done()


if __name__ == "__main__":
    run(TestAgent())
