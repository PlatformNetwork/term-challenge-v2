"""
Term Challenge Protocol Types.
"""

from __future__ import annotations
import json
import re
from dataclasses import dataclass, field
from typing import Optional, List, Dict, Any


@dataclass
class HistoryEntry:
    """A single step in the conversation history."""
    step: int
    command: Optional[str] = None
    output: Optional[str] = None
    exit_code: Optional[int] = None


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
        history: Full conversation history (all previous steps)
    """
    instruction: str
    step: int
    last_command: Optional[str] = None
    output: Optional[str] = None
    exit_code: Optional[int] = None
    cwd: str = "/app"
    history: List[HistoryEntry] = field(default_factory=list)
    
    @classmethod
    def parse(cls, data: str | dict) -> Request:
        """Parse request from JSON string or dict."""
        if isinstance(data, str):
            data = json.loads(data)
        
        # Parse history entries
        history = []
        for entry in data.get("history", []):
            history.append(HistoryEntry(
                step=entry.get("step", 0),
                command=entry.get("command"),
                output=entry.get("output"),
                exit_code=entry.get("exit_code"),
            ))
        
        return cls(
            instruction=data.get("instruction", ""),
            step=data.get("step", 1),
            last_command=data.get("last_command"),
            output=data.get("output"),
            exit_code=data.get("exit_code"),
            cwd=data.get("cwd", "/app"),
            history=history,
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
        text: Text output/message to display
        task_complete: True when task is finished
        data: Additional data to pass back
    
    Example:
        # Execute a command
        Response.cmd("ls -la")
        
        # Send text message
        Response.text("Analyzing the output...")
        
        # Command with text
        Response.cmd("make build").with_text("Building project...")
        
        # Task complete with summary
        Response.done("Task completed successfully!")
    """
    command: Optional[str] = None
    text: Optional[str] = None
    task_complete: bool = False
    data: Optional[Dict[str, Any]] = None
    
    def to_dict(self) -> dict:
        d = {
            "command": self.command,
            "task_complete": self.task_complete,
        }
        if self.text:
            d["text"] = self.text
        if self.data:
            d["data"] = self.data
        return d
    
    def to_json(self) -> str:
        """Convert to JSON string."""
        return json.dumps(self.to_dict())
    
    def with_text(self, text: str) -> Response:
        """Add text to response."""
        self.text = text
        return self
    
    def with_data(self, data: Dict[str, Any]) -> Response:
        """Add data to response."""
        self.data = data
        return self
    
    def complete(self) -> Response:
        """Mark task as complete."""
        self.task_complete = True
        return self
    
    @classmethod
    def cmd(cls, command: str, text: Optional[str] = None) -> Response:
        """Create response with a command."""
        return cls(command=command, text=text, task_complete=False)
    
    @classmethod
    def say(cls, text: str) -> Response:
        """Create response with text only (no command)."""
        return cls(command=None, text=text, task_complete=False)
    
    @classmethod
    def done(cls, text: Optional[str] = None) -> Response:
        """Create response marking task complete."""
        return cls(command=None, text=text, task_complete=True)
    
    @classmethod
    def from_llm(cls, text: str) -> Response:
        """
        Parse response from LLM output.
        
        Extracts JSON from LLM response text.
        """
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
                    text=data.get("text"),
                    task_complete=data.get("task_complete", False),
                    data=data.get("data"),
                )
            except json.JSONDecodeError:
                pass
        
        return cls.done()


@dataclass
class FunctionCall:
    """A function call from the LLM."""
    name: str
    arguments: Dict[str, Any]
    id: Optional[str] = None


@dataclass
class Tool:
    """
    Tool/function definition for LLM.
    
    Example:
        tool = Tool(
            name="search_files",
            description="Search for files matching a pattern",
            parameters={
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Glob pattern"},
                    "directory": {"type": "string", "description": "Directory to search"}
                },
                "required": ["pattern"]
            }
        )
    """
    name: str
    description: str
    parameters: Dict[str, Any] = field(default_factory=dict)
    
    def to_dict(self) -> dict:
        return {
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": self.parameters,
            }
        }
