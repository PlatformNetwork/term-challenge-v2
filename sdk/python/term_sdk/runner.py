"""
Agent runner - Communicates with the Rust harness via stdin/stdout

Usage:
    from term_sdk.runner import run_agent_loop
    from term_sdk import Agent, AgentResponse
    
    class MyAgent(Agent):
        async def step(self, task: str, screen: str) -> AgentResponse:
            ...
    
    if __name__ == "__main__":
        run_agent_loop(MyAgent())
"""

import sys
import json
import asyncio
from typing import TYPE_CHECKING
from dataclasses import asdict

if TYPE_CHECKING:
    from .agent import Agent
    from .protocol import AgentResponse


def run_agent_loop(agent: "Agent"):
    """
    Run the agent in a loop, communicating with the harness via stdin/stdout.
    
    Protocol:
    - Receives JSON on stdin: {"instruction": "...", "screen": "...", "step": 1}
    - Sends JSON on stdout: {"analysis": "...", "plan": "...", "commands": [...], "task_complete": false}
    """
    asyncio.run(_run_agent_loop_async(agent))


async def _run_agent_loop_async(agent: "Agent"):
    """Async implementation of agent loop"""
    
    # Setup agent
    try:
        await agent.setup()
    except Exception as e:
        print(json.dumps({
            "error": f"Agent setup failed: {e}",
            "analysis": "Setup error",
            "plan": "Cannot continue",
            "commands": [],
            "task_complete": True
        }), flush=True)
        return
    
    # Main loop - read from stdin, process, write to stdout
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        
        try:
            # Parse request
            request = json.loads(line)
            instruction = request.get("instruction", "")
            screen = request.get("screen", "")
            step = request.get("step", 1)
            
            # Call agent step
            response = await agent.step(instruction, screen, step)
            
            # Convert response to dict
            if hasattr(response, 'to_dict'):
                response_dict = response.to_dict()
            elif hasattr(response, '__dataclass_fields__'):
                response_dict = asdict(response)
            else:
                response_dict = dict(response)
            
            # Ensure required fields
            response_dict.setdefault("analysis", "")
            response_dict.setdefault("plan", "")
            response_dict.setdefault("commands", [])
            response_dict.setdefault("task_complete", False)
            
            # Convert commands
            commands = []
            for cmd in response_dict.get("commands", []):
                if hasattr(cmd, 'to_dict'):
                    commands.append(cmd.to_dict())
                elif hasattr(cmd, '__dataclass_fields__'):
                    commands.append(asdict(cmd))
                elif isinstance(cmd, dict):
                    commands.append(cmd)
                else:
                    commands.append({"keystrokes": str(cmd), "duration": 1.0})
            response_dict["commands"] = commands
            
            # Output response
            print(json.dumps(response_dict), flush=True)
            
        except json.JSONDecodeError as e:
            print(json.dumps({
                "analysis": f"JSON parse error: {e}",
                "plan": "Invalid request",
                "commands": [],
                "task_complete": False
            }), flush=True)
        except Exception as e:
            print(json.dumps({
                "analysis": f"Agent error: {e}",
                "plan": "Error occurred",
                "commands": [],
                "task_complete": False
            }), flush=True)


def main():
    """Entry point for running agent from command line"""
    import argparse
    
    parser = argparse.ArgumentParser(description="Run a Term Challenge agent")
    parser.add_argument("agent_module", help="Agent module path (e.g., my_agent:MyAgent)")
    args = parser.parse_args()
    
    # Import agent
    if ":" in args.agent_module:
        module_path, class_name = args.agent_module.rsplit(":", 1)
    else:
        module_path = args.agent_module
        class_name = "Agent"
    
    import importlib.util
    import os
    
    # Load module
    if os.path.isfile(module_path) or os.path.isfile(module_path + ".py"):
        # Load from file
        file_path = module_path if module_path.endswith(".py") else module_path + ".py"
        spec = importlib.util.spec_from_file_location("agent_module", file_path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
    else:
        # Load from package
        module = importlib.import_module(module_path)
    
    # Get agent class
    agent_class = getattr(module, class_name)
    agent = agent_class()
    
    # Run loop
    run_agent_loop(agent)


if __name__ == "__main__":
    main()
