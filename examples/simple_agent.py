#!/usr/bin/env python3
"""
Simple Test Agent for Term Challenge (SDK 2.0)

A minimal agent that demonstrates the required structure for the SDK.
This agent doesn't use an LLM - it just performs basic file operations.
"""

from term_sdk import Agent, AgentContext, run


class SimpleAgent(Agent):
    """Simple agent that handles basic file operations."""

    def setup(self):
        """Initialize agent (called once at startup)."""
        pass

    def run(self, ctx: AgentContext):
        """Execute the task."""
        ctx.log(f"Task: {ctx.instruction[:100]}...")
        
        instruction_lower = ctx.instruction.lower()

        # Start by exploring the environment
        result = ctx.shell("ls -la")
        ctx.log(f"Found {len(result.stdout.splitlines())} items")

        # Simple pattern matching for common tasks
        if "hello" in instruction_lower and "file" in instruction_lower:
            ctx.log("Detected: Create hello.txt task")
            ctx.shell('echo "Hello, world!" > hello.txt')
            verify = ctx.shell('cat hello.txt')
            if verify.has("Hello"):
                ctx.log("Task complete: hello.txt created successfully")
        
        elif "list" in instruction_lower or "find" in instruction_lower:
            ctx.log("Detected: File search task")
            ctx.shell("find . -type f 2>/dev/null | head -20")
        
        elif "create" in instruction_lower and "directory" in instruction_lower:
            ctx.log("Detected: Create directory task")
            ctx.shell("mkdir -p output")
            ctx.shell("ls -la")
        
        else:
            # Default: explore more
            ctx.log("Unknown task, exploring...")
            ctx.shell("pwd")
            ctx.shell("cat README.md 2>/dev/null || echo 'No README'")

        ctx.done()

    def cleanup(self):
        """Cleanup (called at shutdown)."""
        pass


if __name__ == "__main__":
    run(SimpleAgent())
