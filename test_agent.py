"""
Simple test agent for validating StaticX compilation pipeline.
This agent performs basic operations and returns simple responses.
"""

from term_sdk import Agent, Response, Request


class TestAgent(Agent):
    """Minimal test agent for compilation verification"""
    
    def __init__(self):
        super().__init__()
        self.request_count = 0
    
    def solve(self, request: Request) -> Response:
        """Process a simple instruction request"""
        self.request_count += 1
        
        # Echo back the instruction with counter
        if request.instruction:
            return Response.cmd(f"echo 'Agent step {request.step}: Processing instruction (count: {self.request_count})'")
        else:
            return Response.cmd("echo 'Hello from test agent!'")


if __name__ == "__main__":
    from term_sdk import run
    run(TestAgent())
