"""
Term Challenge SDK - Build agents that solve terminal tasks.

Quick Start:
    ```python
    from term_sdk import Agent, Request, Response, run

    class MyAgent(Agent):
        def solve(self, req: Request) -> Response:
            if req.step == 1:
                return Response.cmd("ls -la")
            return Response.done()

    if __name__ == "__main__":
        run(MyAgent())
    ```

With LLM:
    ```python
    from term_sdk import Agent, Request, Response, LLM, run

    class LLMAgent(Agent):
        def setup(self):
            self.llm = LLM(model="anthropic/claude-3-haiku")

        def solve(self, req: Request) -> Response:
            result = self.llm.ask(f"Task: {req.instruction}\\nOutput: {req.output}")
            return Response.from_llm(result)

    if __name__ == "__main__":
        run(LLMAgent())
    ```
"""

__version__ = "1.0.0"

from .types import Request, Response
from .agent import Agent
from .runner import run
from .llm import LLM, LLMResponse

__all__ = [
    "Request",
    "Response", 
    "Agent",
    "run",
    "LLM",
    "LLMResponse",
]
