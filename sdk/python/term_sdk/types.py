"""
Term Challenge Protocol Types.

Request: What the harness sends to your agent
Response: What your agent sends back
"""

from __future__ import annotations
import json
import re
from dataclasses import dataclass
from typing import Optional


@dataclass
class Request:
    """
    Request from the harness.
    
    Attributes:
        instruction: The task to complete
        step: Current step number (starts at 1)
        last_command: Previous command you executed (None on step 1)
        output: Output from last command (None on step 1)
        exit_code: Exit code from last command (None on step 1)
        cwd: Current working directory
    
    Example:
        ```python
        def solve(self, req: Request) -> Response:
            if req.step == 1:
                return Response.cmd("pwd")
            
            if req.failed:
                return Response.cmd("echo 'retry'")
            
            if "hello" in req.output:
                return Response.done()
        ```
    """
    instruction: str
    step: int
    last_command: Optional[str] = None
    output: Optional[str] = None
    exit_code: Optional[int] = None
    cwd: str = "/app"
    
    @classmethod
    def parse(cls, data: str | dict) -> Request:
        """Parse request from JSON string or dict."""
        if isinstance(data, str):
            data = json.loads(data)
        return cls(
            instruction=data.get("instruction", ""),
            step=data.get("step", 1),
            last_command=data.get("last_command"),
            output=data.get("output"),
            exit_code=data.get("exit_code"),
            cwd=data.get("cwd", "/app"),
        )
    
    @property
    def first(self) -> bool:
        """True if this is the first step."""
        return self.step == 1
    
    @property
    def ok(self) -> bool:
        """True if last command succeeded (exit_code == 0)."""
        return self.exit_code == 0
    
    @property
    def failed(self) -> bool:
        """True if last command failed (exit_code != 0)."""
        return self.exit_code is not None and self.exit_code != 0
    
    def has(self, *patterns: str) -> bool:
        """Check if output contains any of the patterns."""
        if not self.output:
            return False
        output_lower = self.output.lower()
        return any(p.lower() in output_lower for p in patterns)
    
    def match(self, pattern: str) -> Optional[re.Match]:
        """Match output against regex pattern."""
        if not self.output:
            return None
        return re.search(pattern, self.output)


@dataclass  
class Response:
    """
    Response to the harness.
    
    Attributes:
        command: Shell command to execute (None = no command)
        task_complete: True when task is finished
    
    Example:
        ```python
        # Execute a command
        Response.cmd("ls -la")
        
        # Task complete
        Response.done()
        
        # Execute then complete
        Response.cmd("echo done").complete()
        ```
    """
    command: Optional[str] = None
    task_complete: bool = False
    
    def to_json(self) -> str:
        """Convert to JSON string."""
        return json.dumps({
            "command": self.command,
            "task_complete": self.task_complete,
        })
    
    def complete(self) -> Response:
        """Mark task as complete."""
        self.task_complete = True
        return self
    
    @classmethod
    def cmd(cls, command: str) -> Response:
        """Create response with a command."""
        return cls(command=command, task_complete=False)
    
    @classmethod
    def done(cls) -> Response:
        """Create response marking task complete."""
        return cls(command=None, task_complete=True)
    
    @classmethod
    def from_llm(cls, text: str) -> Response:
        """
        Parse response from LLM output.
        
        Extracts JSON from LLM response text.
        Handles common formats:
        - Raw JSON: {"command": "...", "task_complete": false}
        - JSON in code block: ```json {...} ```
        - Text with embedded JSON
        """
        # Try to find JSON in response
        text = text.strip()
        
        # Remove markdown code blocks
        if "```" in text:
            match = re.search(r'```(?:json)?\s*(\{.*?\})\s*```', text, re.DOTALL)
            if match:
                text = match.group(1)
        
        # Find JSON object
        start = text.find('{')
        end = text.rfind('}')
        
        if start >= 0 and end > start:
            try:
                data = json.loads(text[start:end + 1])
                return cls(
                    command=data.get("command"),
                    task_complete=data.get("task_complete", False),
                )
            except json.JSONDecodeError:
                pass
        
        # Fallback: no valid JSON found
        return cls.done()
