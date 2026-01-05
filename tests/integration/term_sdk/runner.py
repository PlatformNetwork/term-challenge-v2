"""
Runner for term_sdk agents

Keep in sync with compiler.rs create_minimal_sdk_in_container()
"""

import sys
import json
from .types import Request, Response


def run(agent):
    """Run an agent in a loop, reading JSON from stdin and writing JSON to stdout."""
    if hasattr(agent, 'setup'):
        agent.setup()

    for line in sys.stdin:
        try:
            data = json.loads(line.strip())
            req = Request(
                instruction=data.get('instruction', ''),
                step=data.get('step', 1),
                output=data.get('output', ''),
                exit_code=data.get('exit_code', 0),
            )

            resp = agent.solve(req)
            print(json.dumps(resp.to_dict()), flush=True)

            if resp.task_complete:
                break
        except Exception as e:
            print(json.dumps({"command": f"echo ERROR: {e}", "task_complete": False}), flush=True)

    if hasattr(agent, 'cleanup'):
        agent.cleanup()
