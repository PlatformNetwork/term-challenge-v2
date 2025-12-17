#!/usr/bin/env python3
"""
Simple rule-based agent example.
"""
from term_sdk import Agent, Request, Response, run


class SimpleAgent(Agent):
    """Agent that completes basic tasks with rules."""
    
    def solve(self, req: Request) -> Response:
        # First step: explore
        if req.first:
            return Response.cmd("ls -la")
        
        # Check for errors
        if req.failed:
            return Response.cmd("pwd")
        
        # Example: create hello.txt task
        if "hello" in req.instruction.lower():
            if req.step == 2:
                return Response.cmd("echo 'Hello, world!' > hello.txt")
            if req.step == 3:
                return Response.cmd("cat hello.txt")
            if req.has("Hello"):
                return Response.done()
        
        # Default: complete after exploration
        if req.step > 5:
            return Response.done()
        
        return Response.cmd("pwd")


if __name__ == "__main__":
    run(SimpleAgent())
