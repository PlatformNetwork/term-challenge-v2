"""
Terminal interface for Term Challenge agents.

Provides tools to interact with the sandboxed terminal environment.
Compatible with terminal-bench's TmuxSession.
"""

import asyncio
import time
from dataclasses import dataclass, field
from typing import Any, Callable, Dict, List, Optional, Union


@dataclass
class CommandResult:
    """Result of a terminal command execution"""
    output: str
    exit_code: Optional[int] = None
    duration_sec: float = 0.0
    timed_out: bool = False


@dataclass
class TerminalCommand:
    """Command to execute in the terminal"""
    command: str
    block: bool = True
    timeout_sec: float = 60.0
    append_enter: bool = True


class Terminal:
    """
    Terminal interface for interacting with the sandboxed environment.
    
    This class provides a high-level API for terminal operations that is
    compatible with terminal-bench's execution harness.
    
    Usage:
        from term_sdk import Terminal
        
        terminal = Terminal()
        
        # Run a command and get output
        result = await terminal.run("ls -la")
        print(result.output)
        
        # Run command without waiting for output
        await terminal.run("sleep 5 &", block=False)
        
        # Send keystrokes (for interactive programs)
        await terminal.send_keys("vim test.py", "Enter")
        await terminal.send_keys("i")  # Enter insert mode
        await terminal.send_keys("print('hello')", "Escape", ":wq", "Enter")
        
        # Get current screen content
        screen = await terminal.capture_screen()
    """
    
    # tmux-style special keys
    SPECIAL_KEYS = {
        "Enter": "Enter",
        "Tab": "Tab",
        "Escape": "Escape",
        "Backspace": "BSpace",
        "Delete": "DC",
        "Up": "Up",
        "Down": "Down",
        "Left": "Left",
        "Right": "Right",
        "Home": "Home",
        "End": "End",
        "PageUp": "PPage",
        "PageDown": "NPage",
        "Space": "Space",
        "Ctrl-C": "C-c",
        "Ctrl-D": "C-d",
        "Ctrl-Z": "C-z",
        "Ctrl-L": "C-l",
        "Ctrl-A": "C-a",
        "Ctrl-E": "C-e",
        "Ctrl-K": "C-k",
        "Ctrl-U": "C-u",
        "Ctrl-W": "C-w",
        "Ctrl-R": "C-r",
    }
    
    def __init__(
        self,
        session: Any = None,
        default_timeout: float = 60.0,
    ):
        """
        Initialize terminal interface.
        
        Args:
            session: TmuxSession instance (injected by harness during evaluation)
            default_timeout: Default command timeout in seconds
        """
        self._session = session
        self._default_timeout = default_timeout
        self._command_history: List[TerminalCommand] = []
        self._output_history: List[CommandResult] = []
    
    def _is_available(self) -> bool:
        """Check if terminal session is available"""
        return self._session is not None
    
    async def run(
        self,
        command: str,
        block: bool = True,
        timeout_sec: Optional[float] = None,
        append_enter: bool = True,
    ) -> CommandResult:
        """
        Execute a command in the terminal.
        
        Args:
            command: The command to execute
            block: Whether to wait for completion (default: True)
            timeout_sec: Maximum time to wait (default: default_timeout)
            append_enter: Whether to append Enter keystroke (default: True)
            
        Returns:
            CommandResult with output and metadata
        """
        timeout = timeout_sec or self._default_timeout
        
        cmd = TerminalCommand(
            command=command,
            block=block,
            timeout_sec=timeout,
            append_enter=append_enter,
        )
        self._command_history.append(cmd)
        
        if not self._is_available():
            # Simulation mode - return mock result
            return await self._simulate_command(cmd)
        
        # Real execution via TmuxSession
        start_time = time.time()
        try:
            keys = [command]
            if append_enter:
                keys.append("Enter")
            
            self._session.send_keys(
                keys=keys,
                block=block,
                max_timeout_sec=timeout,
            )
            
            duration = time.time() - start_time
            output = self._session.get_incremental_output() if block else ""
            
            result = CommandResult(
                output=output,
                duration_sec=duration,
                timed_out=False,
            )
            
        except TimeoutError:
            result = CommandResult(
                output="",
                duration_sec=timeout,
                timed_out=True,
            )
        
        self._output_history.append(result)
        return result
    
    async def send_keys(self, *keys: str, block: bool = False, timeout_sec: float = 1.0) -> None:
        """
        Send keystrokes to the terminal.
        
        Useful for interactive programs (vim, less, etc.) or special keys.
        
        Args:
            *keys: Keys to send (can use special key names like "Enter", "Ctrl-C")
            block: Whether to wait for command completion
            timeout_sec: Time to wait after sending keys
            
        Example:
            # Enter vim, write code, save and quit
            await terminal.send_keys("vim test.py", "Enter")
            await terminal.send_keys("i")  # Insert mode
            await terminal.send_keys("print('hello')")
            await terminal.send_keys("Escape", ":wq", "Enter")
        """
        processed_keys = []
        for key in keys:
            # Map special key names to tmux format
            if key in self.SPECIAL_KEYS:
                processed_keys.append(self.SPECIAL_KEYS[key])
            elif key.startswith("Ctrl-") and len(key) == 6:
                processed_keys.append(f"C-{key[-1].lower()}")
            else:
                processed_keys.append(key)
        
        if not self._is_available():
            # Simulation mode
            await asyncio.sleep(0.1)
            return
        
        self._session.send_keys(
            keys=processed_keys,
            block=block,
            min_timeout_sec=timeout_sec if not block else 0,
            max_timeout_sec=timeout_sec if block else 180.0,
        )
    
    async def capture_screen(self, full_history: bool = False) -> str:
        """
        Capture current terminal screen content.
        
        Args:
            full_history: If True, capture entire scrollback buffer
            
        Returns:
            Current terminal screen content
        """
        if not self._is_available():
            return "[Terminal screen not available in simulation mode]"
        
        return self._session.capture_pane(capture_entire=full_history)
    
    async def get_output(self) -> str:
        """
        Get new terminal output since last call.
        
        Returns:
            New output or current screen if unable to determine
        """
        if not self._is_available():
            return "[No output available in simulation mode]"
        
        return self._session.get_incremental_output()
    
    async def wait(self, seconds: float) -> None:
        """
        Wait for specified time.
        
        Args:
            seconds: Time to wait in seconds
        """
        await asyncio.sleep(seconds)
    
    async def clear(self) -> None:
        """Clear the terminal screen"""
        await self.run("clear", block=False)
    
    async def _simulate_command(self, cmd: TerminalCommand) -> CommandResult:
        """Simulate command execution for testing"""
        # Simple simulation
        await asyncio.sleep(0.1)
        
        simulated_outputs = {
            "ls": "file1.txt  file2.py  dir1/",
            "pwd": "/home/user",
            "whoami": "user",
            "date": "Mon Dec 16 15:00:00 UTC 2025",
            "echo": cmd.command.replace("echo ", ""),
        }
        
        # Try to match command
        base_cmd = cmd.command.split()[0] if cmd.command else ""
        output = simulated_outputs.get(base_cmd, f"[Simulated output for: {cmd.command}]")
        
        return CommandResult(
            output=output,
            duration_sec=0.1,
            timed_out=False,
        )
    
    @property
    def history(self) -> List[TerminalCommand]:
        """Get command history"""
        return self._command_history.copy()
    
    @property
    def outputs(self) -> List[CommandResult]:
        """Get output history"""
        return self._output_history.copy()


# Convenience functions for common operations

async def run(command: str, **kwargs) -> CommandResult:
    """Run a command using the default terminal"""
    return await get_terminal().run(command, **kwargs)


async def send_keys(*keys: str, **kwargs) -> None:
    """Send keys using the default terminal"""
    await get_terminal().send_keys(*keys, **kwargs)


async def capture_screen(**kwargs) -> str:
    """Capture screen using the default terminal"""
    return await get_terminal().capture_screen(**kwargs)


# Global terminal instance
_terminal: Optional[Terminal] = None


def get_terminal() -> Terminal:
    """Get or create the global terminal instance"""
    global _terminal
    if _terminal is None:
        _terminal = Terminal()
    return _terminal


def set_terminal(terminal: Terminal) -> None:
    """Set the global terminal instance (used by harness)"""
    global _terminal
    _terminal = terminal
