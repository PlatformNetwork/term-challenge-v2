#!/usr/bin/env python3
"""
Code Validator Agent - Validates code correctness in a workspace.

Uses LLM to analyze code and returns a JSON verdict with pass/fail status.
Supports workspace tools: read_file, edit_file, shell.
"""

import argparse
import json
import os
import socket
import ssl
import subprocess
import sys
import urllib.error
import urllib.request
from copy import deepcopy
from pathlib import Path
from typing import Any, Dict, List, Optional


# Configuration from environment (never hardcode API keys)
OPENROUTER_API_KEY = os.environ.get("OPENROUTER_API_KEY", os.environ.get("LLM_API_KEY", ""))
DEFAULT_MODEL = "moonshotai/kimi-k2.5"
DEFAULT_TIMEOUT = 120
OPENROUTER_URL = "https://openrouter.ai/api/v1/chat/completions"

# File extensions to analyze
CODE_EXTENSIONS = {
    ".py", ".rs", ".js", ".ts", ".go", ".java", ".c", ".cpp", ".h", ".hpp",
    ".rb", ".php", ".sh", ".yaml", ".yml", ".json", ".toml", ".md"
}

# Maximum file size to read (to avoid context overflow)
MAX_FILE_SIZE = 50000  # 50KB


# =============================================================================
# Caching Module (inline from term_sdk.caching)
# =============================================================================

def _normalize_content(content) -> List[dict]:
    """Normalize message content to array format."""
    if isinstance(content, str):
        return [{"type": "text", "text": content}]
    if isinstance(content, list):
        return content
    return [{"type": "text", "text": str(content)}]


def _add_cache_control(message: dict) -> dict:
    """Add cache_control to a message's content blocks."""
    result = deepcopy(message)
    content_blocks = _normalize_content(result.get("content", ""))
    for block in content_blocks:
        if isinstance(block, dict) and block.get("type") == "text":
            block["cache_control"] = {"type": "ephemeral"}
    result["content"] = content_blocks
    return result


def apply_caching(messages: List[dict], enabled: bool = True) -> List[dict]:
    """Apply Anthropic's 4-breakpoint caching strategy to messages."""
    if not enabled or not messages:
        return deepcopy(messages) if messages else []

    result = deepcopy(messages)

    # Find indices of system messages (first 2)
    system_indices = []
    for i, msg in enumerate(result):
        if msg.get("role") == "system":
            system_indices.append(i)
            if len(system_indices) >= 2:
                break

    # Find indices of non-system messages (last 2)
    non_system_indices = []
    for i in range(len(result) - 1, -1, -1):
        if result[i].get("role") != "system":
            non_system_indices.insert(0, i)
            if len(non_system_indices) >= 2:
                break

    # Combine indices to cache (max 4 total)
    cache_indices = set(system_indices + non_system_indices)

    # Apply cache_control to selected indices
    for i in cache_indices:
        result[i] = _add_cache_control(result[i])

    return result


# =============================================================================
# Tool Functions
# =============================================================================

def shell(cmd: str, cwd: Optional[str] = None, timeout: int = 60) -> dict:
    """
    Execute shell command and return result.

    Args:
        cmd: Command to execute
        cwd: Working directory (optional)
        timeout: Command timeout in seconds

    Returns:
        dict with stdout, stderr, exit_code, timed_out
    """
    try:
        proc = subprocess.run(
            cmd,
            shell=True,
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=timeout
        )
        return {
            "stdout": proc.stdout,
            "stderr": proc.stderr,
            "exit_code": proc.returncode,
            "timed_out": False,
            "ok": proc.returncode == 0
        }
    except subprocess.TimeoutExpired:
        return {
            "stdout": "",
            "stderr": f"Command timed out after {timeout}s",
            "exit_code": -1,
            "timed_out": True,
            "ok": False
        }
    except Exception as exc:
        return {
            "stdout": "",
            "stderr": str(exc),
            "exit_code": -1,
            "timed_out": False,
            "ok": False
        }


def read_file(path: str) -> str:
    """
    Read file contents.

    Args:
        path: Path to file

    Returns:
        File contents as string, or error message
    """
    try:
        file_path = Path(path)
        if not file_path.exists():
            return f"[ERROR] File not found: {path}"
        if file_path.stat().st_size > MAX_FILE_SIZE:
            return f"[ERROR] File too large (>{MAX_FILE_SIZE} bytes): {path}"
        return file_path.read_text(encoding="utf-8", errors="replace")
    except Exception as exc:
        return f"[ERROR] Failed to read file: {exc}"


def edit_file(path: str, content: str) -> bool:
    """
    Write content to file.

    Args:
        path: Path to file
        content: Content to write

    Returns:
        True if successful, False otherwise
    """
    try:
        file_path = Path(path)
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(content, encoding="utf-8")
        return True
    except Exception:
        return False


# =============================================================================
# LLM Client
# =============================================================================

class LLMError(Exception):
    """Exception raised for LLM API errors."""

    def __init__(self, code: str, message: str):
        self.code = code
        self.message = message
        super().__init__(f"[{code}] {message}")


def call_llm(
    messages: List[dict],
    model: str = DEFAULT_MODEL,
    temperature: float = 0.3,
    max_tokens: int = 4096,
    enable_caching: bool = True
) -> str:
    """
    Call LLM via OpenRouter API with caching support.

    Args:
        messages: List of message dicts with 'role' and 'content' keys
        model: Model to use
        temperature: Sampling temperature
        max_tokens: Maximum response tokens
        enable_caching: Whether to apply prompt caching

    Returns:
        Response text from the model

    Raises:
        LLMError: On API errors
    """
    if not OPENROUTER_API_KEY:
        raise LLMError(
            "missing_api_key",
            "No API key. Set OPENROUTER_API_KEY or LLM_API_KEY environment variable."
        )

    # Apply caching if enabled
    if enable_caching:
        messages = apply_caching(messages)

    body = {
        "model": model,
        "messages": messages,
        "temperature": temperature,
        "max_tokens": max_tokens,
        "stream": False,
    }

    data = json.dumps(body).encode("utf-8")
    ssl_context = ssl.create_default_context()

    request = urllib.request.Request(
        OPENROUTER_URL,
        data=data,
        headers={
            "Authorization": f"Bearer {OPENROUTER_API_KEY}",
            "Content-Type": "application/json",
        },
        method="POST",
    )

    try:
        with urllib.request.urlopen(
            request, timeout=DEFAULT_TIMEOUT, context=ssl_context
        ) as response:
            response_data = response.read().decode("utf-8")
            result = json.loads(response_data)
    except urllib.error.HTTPError as exc:
        try:
            error_body = exc.read().decode("utf-8")
            error_data = json.loads(error_body)
            if "error" in error_data:
                error = error_data["error"]
                raise LLMError(
                    error.get("code", f"http_{exc.code}"),
                    error.get("message", str(exc)),
                ) from exc
        except (ValueError, KeyError, json.JSONDecodeError):
            pass
        raise LLMError(f"http_{exc.code}", f"HTTP error: {exc.code}") from exc
    except urllib.error.URLError as exc:
        if isinstance(exc.reason, socket.timeout):
            raise LLMError("timeout", f"Request timed out after {DEFAULT_TIMEOUT}s") from exc
        raise LLMError("request_error", f"Request failed: {exc.reason}") from exc
    except socket.timeout as exc:
        raise LLMError("timeout", f"Request timed out after {DEFAULT_TIMEOUT}s") from exc
    except json.JSONDecodeError as exc:
        raise LLMError("parse_error", f"Failed to parse response: {exc}") from exc

    # Check for error response
    if "error" in result:
        error = result["error"]
        raise LLMError(
            error.get("code", "api_error") if isinstance(error, dict) else "api_error",
            error.get("message", str(error)) if isinstance(error, dict) else str(error)
        )

    # Extract response content
    choices = result.get("choices", [])
    if not choices:
        raise LLMError("empty_response", "No choices in response")

    message = choices[0].get("message", {})
    return message.get("content", "")


# =============================================================================
# Validation Logic
# =============================================================================

def discover_files(workspace: str, extensions: set = None) -> List[Path]:
    """
    Discover code files in workspace.

    Args:
        workspace: Path to workspace directory
        extensions: Set of file extensions to include

    Returns:
        List of file paths
    """
    if extensions is None:
        extensions = CODE_EXTENSIONS

    workspace_path = Path(workspace)
    if not workspace_path.exists():
        return []

    files = []
    for ext in extensions:
        files.extend(workspace_path.rglob(f"*{ext}"))

    # Filter out hidden directories and common excludes
    exclude_patterns = {".git", "__pycache__", "node_modules", ".venv", "venv", "target", "dist"}
    filtered = []
    for f in files:
        if not any(part.startswith(".") or part in exclude_patterns for part in f.parts):
            filtered.append(f)

    return sorted(filtered, key=lambda p: p.stat().st_size, reverse=True)[:20]  # Top 20 largest


def build_file_context(files: List[Path], workspace: str) -> str:
    """Build file context string for LLM analysis."""
    context_parts = []
    total_size = 0
    max_context_size = 100000  # 100KB total context

    workspace_path = Path(workspace).resolve()

    for file_path in files:
        if total_size >= max_context_size:
            break

        try:
            relative_path = file_path.relative_to(workspace_path)
        except ValueError:
            relative_path = file_path.name

        content = read_file(str(file_path))
        if content.startswith("[ERROR]"):
            continue

        file_section = f"\n### File: {relative_path}\n```\n{content}\n```\n"
        section_size = len(file_section)

        if total_size + section_size > max_context_size:
            # Truncate this file
            remaining = max_context_size - total_size - 100
            if remaining > 500:
                content = content[:remaining] + "\n... [truncated]"
                file_section = f"\n### File: {relative_path}\n```\n{content}\n```\n"
            else:
                break

        context_parts.append(file_section)
        total_size += len(file_section)

    return "".join(context_parts)


def validate_code(
    workspace: str,
    validation_rules: Optional[List[str]] = None,
    instruction: Optional[str] = None
) -> dict:
    """
    Main validation logic.

    Args:
        workspace: Path to workspace directory
        validation_rules: Optional list of custom validation rules
        instruction: Optional custom instruction for validation

    Returns:
        Validation result dict with pass/fail status and details
    """
    workspace_path = Path(workspace).resolve()

    # Check workspace exists
    if not workspace_path.exists():
        return {
            "passed": False,
            "score": 0.0,
            "summary": f"Workspace not found: {workspace}",
            "issues": [
                {
                    "severity": "error",
                    "file": None,
                    "line": None,
                    "message": f"Workspace directory does not exist: {workspace}"
                }
            ],
            "details": {
                "files_analyzed": 0,
                "security_check": "skipped",
                "code_quality": "skipped",
                "compliance": "skipped"
            }
        }

    # Discover files
    files = discover_files(str(workspace_path))

    if not files:
        return {
            "passed": True,
            "score": 1.0,
            "summary": "No code files found in workspace to validate",
            "issues": [],
            "details": {
                "files_analyzed": 0,
                "security_check": "not_applicable",
                "code_quality": "not_applicable",
                "compliance": "not_applicable"
            }
        }

    # Build file context
    file_context = build_file_context(files, str(workspace_path))

    # Build validation prompt
    rules_text = ""
    if validation_rules:
        rules_text = "\n\nAdditional validation rules:\n" + "\n".join(f"- {rule}" for rule in validation_rules)

    instruction_text = instruction or "Analyze the code for quality, security, and correctness."

    system_prompt = """You are a code validator agent. Analyze the provided code and return a JSON validation result.

Your analysis should check for:
1. Security issues (hardcoded secrets, SQL injection, XSS, unsafe operations)
2. Code quality (error handling, code organization, naming conventions)
3. Correctness (logic errors, edge cases, incomplete implementations)
4. Compliance (follows best practices, no placeholders/TODOs in production code)

Return ONLY valid JSON in this exact format:
{
  "passed": true/false,
  "score": 0.0-1.0,
  "summary": "Brief overall assessment",
  "issues": [
    {
      "severity": "error|warning|info",
      "file": "filename.py",
      "line": 42,
      "message": "Description of the issue"
    }
  ],
  "details": {
    "security_check": "passed|failed|warning",
    "code_quality": "excellent|good|fair|poor",
    "compliance": "passed|failed"
  }
}

Be thorough but fair. Only flag actual issues, not style preferences."""

    user_prompt = f"""{instruction_text}{rules_text}

## Files to Analyze

{file_context}

Analyze the code above and return your validation result as JSON."""

    messages = [
        {"role": "system", "content": system_prompt},
        {"role": "user", "content": user_prompt}
    ]

    # Call LLM for analysis
    try:
        response_text = call_llm(messages)
    except LLMError as exc:
        return {
            "passed": False,
            "score": 0.0,
            "summary": f"LLM analysis failed: {exc.message}",
            "issues": [
                {
                    "severity": "error",
                    "file": None,
                    "line": None,
                    "message": f"LLM error [{exc.code}]: {exc.message}"
                }
            ],
            "details": {
                "files_analyzed": len(files),
                "security_check": "error",
                "code_quality": "error",
                "compliance": "error"
            }
        }

    # Parse LLM response
    try:
        # Extract JSON from response (handle markdown code blocks)
        json_text = response_text
        if "```json" in json_text:
            json_text = json_text.split("```json")[1].split("```")[0]
        elif "```" in json_text:
            json_text = json_text.split("```")[1].split("```")[0]

        result = json.loads(json_text.strip())

        # Validate required fields
        if "passed" not in result:
            result["passed"] = False
        if "score" not in result:
            result["score"] = 1.0 if result["passed"] else 0.0
        if "summary" not in result:
            result["summary"] = "Validation completed"
        if "issues" not in result:
            result["issues"] = []
        if "details" not in result:
            result["details"] = {}

        # Ensure details has required fields
        result["details"].setdefault("files_analyzed", len(files))
        result["details"].setdefault("security_check", "passed")
        result["details"].setdefault("code_quality", "good")
        result["details"].setdefault("compliance", "passed")

        return result

    except (json.JSONDecodeError, IndexError, KeyError) as exc:
        # LLM returned non-JSON response, try to extract meaning
        return {
            "passed": False,
            "score": 0.5,
            "summary": f"Could not parse LLM response: {str(exc)[:100]}",
            "issues": [
                {
                    "severity": "warning",
                    "file": None,
                    "line": None,
                    "message": f"LLM response parsing failed. Raw response: {response_text[:500]}"
                }
            ],
            "details": {
                "files_analyzed": len(files),
                "security_check": "unknown",
                "code_quality": "unknown",
                "compliance": "unknown"
            }
        }


# =============================================================================
# Main Entry Point
# =============================================================================

def main():
    """Main entry point for the Code Validator Agent."""
    parser = argparse.ArgumentParser(
        description="Code Validator Agent - Validates code correctness in a workspace"
    )
    parser.add_argument(
        "--instruction",
        required=True,
        help="Validation instruction or description of what to check"
    )
    parser.add_argument(
        "--workspace",
        default=".",
        help="Workspace directory to validate (default: current directory)"
    )
    parser.add_argument(
        "--output",
        default="validation_result.json",
        help="Output file for results (default: validation_result.json)"
    )
    parser.add_argument(
        "--rules",
        nargs="*",
        default=None,
        help="Additional validation rules to apply"
    )

    args = parser.parse_args()

    # Run validation
    result = validate_code(
        workspace=args.workspace,
        validation_rules=args.rules,
        instruction=args.instruction
    )

    # Save result to file
    output_path = Path(args.output)
    try:
        output_path.write_text(json.dumps(result, indent=2), encoding="utf-8")
    except Exception as exc:
        print(f"Warning: Could not save to {args.output}: {exc}", file=sys.stderr)

    # Print verdict to stdout
    print(json.dumps(result, indent=2))

    # Exit with appropriate code
    sys.exit(0 if result.get("passed", False) else 1)


if __name__ == "__main__":
    main()
