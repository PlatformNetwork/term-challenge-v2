"""
Term Challenge Protocol - Response format compatible with terminal-bench.

This module defines the protocol for agent responses that are compatible
with both Term Challenge and Terminal-Bench harnesses.
"""

import json
from dataclasses import dataclass, field
from typing import List, Optional, Union
from enum import Enum


class ResponseFormat(Enum):
    """Supported response formats"""
    JSON = "json"
    XML = "xml"


@dataclass
class Command:
    """
    A single command to execute in the terminal.
    
    Attributes:
        keystrokes: Exact text to send to the terminal. Include '\\n' to execute.
        duration: Seconds to wait after sending (0.1 - 60.0). Default: 1.0
    
    Examples:
        # Simple command
        Command("ls -la\\n", 0.1)
        
        # Long-running command
        Command("make build\\n", 30.0)
        
        # Special key
        Command("C-c", 0.1)  # Ctrl+C
        
        # Interactive (vim)
        Command("i", 0.1)  # Enter insert mode
    """
    keystrokes: str
    duration: float = 1.0
    
    def __post_init__(self):
        if self.duration < 0:
            self.duration = 0.1
        if self.duration > 60:
            self.duration = 60.0
    
    def to_dict(self) -> dict:
        return {
            "keystrokes": self.keystrokes,
            "duration": self.duration
        }
    
    @classmethod
    def from_dict(cls, data: dict) -> "Command":
        return cls(
            keystrokes=data.get("keystrokes", ""),
            duration=float(data.get("duration", 1.0))
        )


@dataclass
class AgentResponse:
    """
    Agent response compatible with terminal-bench protocol.
    
    This is the main response format that agents must produce.
    
    Attributes:
        analysis: Your analysis of the current terminal state
        plan: Your plan for the next steps
        commands: List of commands to execute
        task_complete: Whether the task is finished (requires double confirmation)
    
    Example:
        response = AgentResponse(
            analysis="I see an empty directory. Need to create hello.py",
            plan="1) Create file with echo, 2) Verify with cat, 3) Run with python",
            commands=[
                Command("echo 'print(42)' > hello.py\\n", 0.1),
                Command("python3 hello.py\\n", 0.5),
            ],
            task_complete=False
        )
        print(response.to_json())
    """
    analysis: str
    plan: str
    commands: List[Command] = field(default_factory=list)
    task_complete: bool = False
    
    def to_dict(self) -> dict:
        return {
            "analysis": self.analysis,
            "plan": self.plan,
            "commands": [cmd.to_dict() for cmd in self.commands],
            "task_complete": self.task_complete
        }
    
    def to_json(self, indent: int = 2) -> str:
        """Convert to JSON string (terminal-bench compatible)"""
        return json.dumps(self.to_dict(), indent=indent)
    
    def to_xml(self) -> str:
        """Convert to XML string (terminal-bench compatible)"""
        commands_xml = "\n".join([
            f"""<command>
<keystrokes>{cmd.keystrokes}</keystrokes>
<duration>{cmd.duration}</duration>
</command>"""
            for cmd in self.commands
        ])
        
        return f"""<response>
<analysis>
{self.analysis}
</analysis>
<plan>
{self.plan}
</plan>
<commands>
{commands_xml}
</commands>
<task_complete>{str(self.task_complete).lower()}</task_complete>
</response>"""
    
    @classmethod
    def from_dict(cls, data: dict) -> "AgentResponse":
        commands = [
            Command.from_dict(cmd) if isinstance(cmd, dict) else cmd
            for cmd in data.get("commands", [])
        ]
        return cls(
            analysis=data.get("analysis", ""),
            plan=data.get("plan", ""),
            commands=commands,
            task_complete=data.get("task_complete", False)
        )
    
    @classmethod
    def from_json(cls, json_str: str) -> "AgentResponse":
        """Parse from JSON string"""
        data = json.loads(json_str)
        return cls.from_dict(data)


# Convenience functions for creating commands

def cmd(keystrokes: str, duration: float = 1.0) -> Command:
    """Create a command (shorthand)"""
    return Command(keystrokes, duration)


def run(command: str, duration: float = 0.5) -> Command:
    """Create a command that executes (appends \\n)"""
    if not command.endswith("\n"):
        command += "\n"
    return Command(command, duration)


def keys(*keystrokes: str, duration: float = 0.1) -> List[Command]:
    """Create multiple keystroke commands"""
    return [Command(k, duration) for k in keystrokes]


def wait(seconds: float = 5.0) -> Command:
    """Wait without sending any keystrokes (for polling)"""
    return Command("", seconds)


# Special key constants
class Keys:
    """Special key constants for terminal interaction"""
    ENTER = "\n"
    CTRL_C = "C-c"
    CTRL_D = "C-d"
    CTRL_Z = "C-z"
    CTRL_L = "C-l"
    CTRL_A = "C-a"
    CTRL_E = "C-e"
    CTRL_K = "C-k"
    CTRL_U = "C-u"
    CTRL_W = "C-w"
    CTRL_R = "C-r"
    ESCAPE = "Escape"
    TAB = "Tab"
    BACKSPACE = "BSpace"
    DELETE = "DC"
    UP = "Up"
    DOWN = "Down"
    LEFT = "Left"
    RIGHT = "Right"
    HOME = "Home"
    END = "End"
    PAGE_UP = "PPage"
    PAGE_DOWN = "NPage"
