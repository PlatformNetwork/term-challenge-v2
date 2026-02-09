"""Security Auditor Agent - Checks for malicious/obfuscated code."""

import base64
import re
from typing import List, Tuple

from .base import CodeAnalysis, ReviewAgent, ReviewMessage, ReviewVerdict


class SecurityAuditor(ReviewAgent):
    """Agent specialized in detecting security issues and obfuscated code."""

    # Patterns for obfuscated/encoded code
    OBFUSCATION_PATTERNS: List[Tuple[str, str]] = [
        (r'exec\s*\(\s*["\']', "Direct exec() with string"),
        (r'eval\s*\(\s*["\']', "Direct eval() with string"),
        (r'compile\s*\(', "compile() usage - potential code injection"),
        (r'__import__\s*\(', "Dynamic import - potential obfuscation"),
        (r'getattr\s*\(\s*__builtins__', "Accessing builtins via getattr"),
        (r'base64\.(b64decode|decode)', "Base64 decoding - check for hidden code"),
        (r'codecs\.(decode|encode)', "Codecs usage - potential obfuscation"),
        (r'\\x[0-9a-fA-F]{2}', "Hex-encoded strings"),
        (r'chr\s*\(\s*\d+\s*\)', "chr() calls - potential string obfuscation"),
        (r'ord\s*\([^)]+\)\s*\^', "XOR obfuscation pattern"),
        (r'lambda\s*:\s*None', "Suspicious lambda"),
        (r'type\s*\(\s*["\']', "Dynamic type creation"),
        (r'["\'][A-Za-z0-9+/]{50,}={0,2}["\']', "Potential base64 encoded string"),
        (r'\\u[0-9a-fA-F]{4}', "Unicode escape sequences"),
        (r'zlib\.(decompress|compress)', "Compression - potential code hiding"),
        (r'marshal\.(loads|dumps)', "Marshal - bytecode serialization"),
        (r'pickle\.(loads|load)', "Pickle - deserialization risk"),
    ]

    # Dangerous operations (warnings, not auto-reject)
    DANGEROUS_PATTERNS: List[Tuple[str, str]] = [
        (r'os\.system\s*\(', "os.system() - shell command execution"),
        (r'subprocess\.Popen.*shell\s*=\s*True', "subprocess with shell=True"),
        (r'socket\.(socket|connect|bind)', "Direct socket operations"),
        (r'urllib\.request\.urlopen', "Direct URL access"),
        (r'requests\.(get|post|put|delete)', "HTTP requests library"),
        (r'ctypes\.', "ctypes - low-level memory access"),
        (r'multiprocessing\.(Process|Pool)', "Multiprocessing usage"),
        (r'threading\.(Thread|Lock)', "Threading operations"),
        (r'open\s*\([^)]*["\']w["\']', "File write operations"),
        (r'shutil\.(rmtree|remove)', "Destructive file operations"),
    ]

    def __init__(self, llm_client=None):
        """Initialize the Security Auditor agent."""
        super().__init__(
            name="SecurityAuditor",
            role="Security & Obfuscation Detection Specialist",
            llm_client=llm_client,
        )

    def analyze_code(self, code: str, filename: str = "agent.py") -> CodeAnalysis:
        """
        Analyze code for security issues and obfuscation.

        Args:
            code: The source code to analyze.
            filename: Name of the file being analyzed.

        Returns:
            CodeAnalysis with security findings.
        """
        issues: List[str] = []
        warnings: List[str] = []
        positives: List[str] = []

        lines = code.split("\n")

        # Check for obfuscation patterns
        for pattern, description in self.OBFUSCATION_PATTERNS:
            for i, line in enumerate(lines, 1):
                if re.search(pattern, line):
                    issues.append(f"Line {i}: {description}")

        # Check for dangerous patterns (warnings, not rejections)
        for pattern, description in self.DANGEROUS_PATTERNS:
            for i, line in enumerate(lines, 1):
                if re.search(pattern, line):
                    warnings.append(f"Line {i}: {description} - verify legitimate use")

        # Check for base64 encoded content that might be code
        base64_strings = re.findall(r'["\']([A-Za-z0-9+/]{40,}={0,2})["\']', code)
        for b64_str in base64_strings:
            try:
                decoded = base64.b64decode(b64_str).decode("utf-8", errors="ignore")
                code_keywords = ["import", "def ", "class ", "exec", "eval"]
                if any(kw in decoded.lower() for kw in code_keywords):
                    issues.append(
                        f"Detected base64-encoded Python code: {b64_str[:30]}..."
                    )
            except Exception:
                pass

        # Check for very long single lines (potential obfuscation)
        for i, line in enumerate(lines, 1):
            if len(line) > 500 and not line.strip().startswith("#"):
                warnings.append(
                    f"Line {i}: Very long line ({len(line)} chars) - potential obfuscation"
                )

        # Check for excessive use of single-char variable names
        single_char_vars = re.findall(r"\b([a-z])\s*=", code)
        if len(single_char_vars) > 20:
            warnings.append(
                f"Excessive single-character variables ({len(single_char_vars)}) - "
                "may indicate obfuscation"
            )

        # Positive signals
        if "term_sdk" in code or "from term_sdk import" in code:
            positives.append("Uses official term_sdk")

        if re.search(r"def\s+(setup|solve|run|cleanup)\s*\(", code):
            positives.append("Has standard agent methods")

        if '"""' in code or "'''" in code:
            positives.append("Has docstrings")

        if re.search(r"#.*[A-Za-z]", code):
            positives.append("Has inline comments")

        # Determine verdict
        if issues:
            verdict = ReviewVerdict.REJECT
            confidence = min(0.9, 0.5 + len(issues) * 0.1)
        elif warnings:
            verdict = ReviewVerdict.NEEDS_DISCUSSION
            confidence = 0.6
        else:
            verdict = ReviewVerdict.APPROVE
            confidence = 0.8

        return CodeAnalysis(
            issues=issues,
            warnings=warnings,
            positives=positives,
            verdict=verdict,
            confidence=confidence,
        )

    def respond_to_discussion(
        self,
        code: str,
        conversation: List[ReviewMessage],
        my_analysis: CodeAnalysis,
    ) -> ReviewMessage:
        """
        Respond to discussion based on other agents' input.

        Args:
            code: The source code being reviewed.
            conversation: List of messages in the discussion so far.
            my_analysis: This agent's initial analysis.

        Returns:
            A ReviewMessage with the agent's response.
        """
        # Find last message not from us
        other_messages = [m for m in conversation if m.agent_name != self.name]

        if not other_messages:
            # First response - state our analysis
            content = self._format_initial_response(my_analysis)
            return self._create_message(content, my_analysis.verdict)

        last_msg = other_messages[-1]

        # If using LLM, generate contextual response
        if self.llm_client:
            return self._llm_respond(code, conversation, my_analysis, last_msg)

        # Rule-based response
        return self._rule_based_respond(my_analysis, last_msg)

    def _format_initial_response(self, analysis: CodeAnalysis) -> str:
        """
        Format initial analysis into a readable message.

        Args:
            analysis: The code analysis results.

        Returns:
            Formatted message string.
        """
        parts = ["## Security Analysis\n"]

        if analysis.issues:
            parts.append("**🚨 Critical Issues Found:**")
            for issue in analysis.issues:
                parts.append(f"- {issue}")
            parts.append("")

        if analysis.warnings:
            parts.append("**⚠️ Warnings:**")
            for warning in analysis.warnings:
                parts.append(f"- {warning}")
            parts.append("")

        if analysis.positives:
            parts.append("**✅ Positive Signals:**")
            for positive in analysis.positives:
                parts.append(f"- {positive}")
            parts.append("")

        verdict_emoji = {
            ReviewVerdict.APPROVE: "✅",
            ReviewVerdict.REJECT: "❌",
            ReviewVerdict.NEEDS_DISCUSSION: "🤔",
        }

        parts.append(
            f"**Verdict:** {verdict_emoji[analysis.verdict]} {analysis.verdict.value}"
        )
        parts.append(f"**Confidence:** {analysis.confidence:.0%}")

        return "\n".join(parts)

    def _rule_based_respond(
        self, analysis: CodeAnalysis, last_msg: ReviewMessage
    ) -> ReviewMessage:
        """
        Generate rule-based response without LLM.

        Args:
            analysis: This agent's analysis.
            last_msg: The last message from another agent.

        Returns:
            A ReviewMessage with the response.
        """
        verdict = analysis.verdict

        # If other agent approved but we found issues, maintain rejection
        if last_msg.verdict == ReviewVerdict.APPROVE and analysis.issues:
            content = (
                f"I understand @{last_msg.agent_name}'s perspective, but I maintain my "
                f"concerns about security. The following issues cannot be ignored:\n\n"
                + "\n".join(f"- {i}" for i in analysis.issues[:3])
            )
            verdict = ReviewVerdict.REJECT

        # If other agent rejected but we found it safe, express support with caution
        elif last_msg.verdict == ReviewVerdict.REJECT and not analysis.issues:
            content = (
                f"From a pure security standpoint, I found no critical issues. "
                f"However, I defer to @{last_msg.agent_name}'s concerns in their domain."
            )
            verdict = ReviewVerdict.NEEDS_DISCUSSION

        # General acknowledgment
        else:
            content = (
                f"Acknowledged @{last_msg.agent_name}'s points. "
                f"My security assessment stands: {len(analysis.issues)} critical issues, "
                f"{len(analysis.warnings)} warnings."
            )

        return self._create_message(content, verdict, last_msg.agent_name)

    def _llm_respond(
        self,
        code: str,
        conversation: List[ReviewMessage],
        analysis: CodeAnalysis,
        last_msg: ReviewMessage,
    ) -> ReviewMessage:
        """
        Generate LLM-powered response.

        Args:
            code: The source code being reviewed.
            conversation: List of messages in the discussion.
            analysis: This agent's analysis.
            last_msg: The last message from another agent.

        Returns:
            A ReviewMessage with the LLM-generated response.
        """
        # Format conversation for LLM
        conv_text = "\n\n".join(
            [
                f"**{m.agent_name}** ({m.verdict.value if m.verdict else 'N/A'}):\n{m.content}"
                for m in conversation[-5:]  # Last 5 messages
            ]
        )

        prompt = f"""You are the Security Auditor in a code review discussion.
Your analysis found:
- Issues: {analysis.issues}
- Warnings: {analysis.warnings}
- Positives: {analysis.positives}

Recent conversation:
{conv_text}

Last message was from {last_msg.agent_name}. Respond thoughtfully:
1. Acknowledge their points
2. Maintain/adjust your security assessment
3. Provide specific evidence from code
4. End with your current verdict: APPROVE, REJECT, or NEEDS_DISCUSSION

Keep response under 200 words. Be professional but direct."""

        try:
            response = self.llm_client.chat(
                [
                    {"role": "system", "content": self.get_system_prompt()},
                    {"role": "user", "content": prompt},
                ]
            )

            # Extract verdict from response
            verdict = analysis.verdict
            response_text = response if isinstance(response, str) else str(response)
            if "APPROVE" in response_text.upper():
                verdict = ReviewVerdict.APPROVE
            elif "REJECT" in response_text.upper():
                verdict = ReviewVerdict.REJECT

            return self._create_message(response_text, verdict, last_msg.agent_name)
        except Exception:
            return self._rule_based_respond(analysis, last_msg)

    def get_system_prompt(self) -> str:
        """Get the system prompt for this agent's LLM."""
        return """You are a Security Auditor AI specialized in detecting:
- Obfuscated or encoded malicious code
- Dangerous operations (file system, network, process execution)
- Code injection vulnerabilities
- Sandbox escape attempts

You are reviewing Python code submissions for an AI agent competition.
Be thorough but fair - flag real security issues, not coding style preferences.
Your job is to protect the platform from malicious submissions."""
