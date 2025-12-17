"""
Agent runner for Term Challenge.
"""

import sys
import json
import traceback
from .types import Request, Response
from .agent import Agent


def log(msg: str) -> None:
    """Log to stderr (stdout is reserved for protocol)."""
    print(f"[agent] {msg}", file=sys.stderr)


def run(agent: Agent) -> None:
    """
    Run an agent in the Term Challenge harness.
    
    This reads requests from stdin and writes responses to stdout.
    
    Args:
        agent: Your agent instance
    
    Example:
        ```python
        from term_sdk import Agent, Request, Response, run
        
        class MyAgent(Agent):
            def solve(self, req: Request) -> Response:
                return Response.cmd("ls")
        
        if __name__ == "__main__":
            run(MyAgent())
        ```
    """
    try:
        # Setup
        agent.setup()
        
        # Read single request from stdin
        input_data = sys.stdin.read().strip()
        if not input_data:
            log("No input received")
            return
        
        # Parse request
        request = Request.parse(input_data)
        log(f"Step {request.step}: {request.instruction[:50]}...")
        
        # Solve
        response = agent.solve(request)
        
        # Output response
        print(response.to_json(), flush=True)
        
        # Cleanup
        agent.cleanup()
        
    except json.JSONDecodeError as e:
        log(f"Invalid JSON: {e}")
        print(Response.done().to_json())
    except Exception as e:
        log(f"Error: {e}")
        traceback.print_exc(file=sys.stderr)
        print(Response.done().to_json())


def run_loop(agent: Agent) -> None:
    """
    Run agent in continuous loop mode (for testing).
    
    Reads multiple requests, one per line.
    """
    try:
        agent.setup()
        
        for line in sys.stdin:
            line = line.strip()
            if not line:
                continue
            
            try:
                request = Request.parse(line)
                response = agent.solve(request)
                print(response.to_json(), flush=True)
                
                if response.task_complete:
                    break
            except Exception as e:
                log(f"Error: {e}")
                print(Response.done().to_json(), flush=True)
                break
        
        agent.cleanup()
        
    except KeyboardInterrupt:
        log("Interrupted")
    except Exception as e:
        log(f"Fatal: {e}")
        traceback.print_exc(file=sys.stderr)
