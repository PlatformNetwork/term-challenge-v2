"""Rule Compliance Verifier Agent - Checks against AGENTS.md rules."""
import re
from typing import List, Set
from .base import ReviewAgent, CodeAnalysis, ReviewVerdict, ReviewMessage


class RuleComplianceVerifier(ReviewAgent):
    """Agent specialized in verifying compliance with term-challenge rules."""
    
    def __init__(self, llm_client=None):
        super().__init__(
            name="RuleComplianceVerifier",
            role="Term-Challenge Rules Compliance Specialist",
            llm_client=llm_client
        )
        
        # SDK patterns that should be present
        self.valid_sdk_patterns = [
            # term_sdk patterns (SDK 2.0)
            (r'from\s+term_sdk\s+import', "term_sdk import"),
            (r'class\s+\w+\s*\(\s*Agent\s*\)', "Agent class inheritance"),
            (r'def\s+run\s*\(\s*self\s*,\s*ctx', "run(self, ctx) method"),
            (r'ctx\.shell\s*\(', "ctx.shell() usage"),
            (r'ctx\.done\s*\(', "ctx.done() call"),
            # argparse patterns (SDK 3.0)
            (r'import\s+argparse', "argparse import"),
            (r'argparse\.ArgumentParser', "ArgumentParser usage"),
            (r'--instruction', "instruction argument"),
            (r'subprocess\.(run|Popen|call)', "subprocess usage"),
        ]
        
        # Forbidden modules for direct network access
        self.forbidden_network_modules = {
            'socket': "Direct socket access not allowed",
            'urllib': "urllib not allowed - use LLM proxy",
            'urllib2': "urllib2 not allowed", 
            'urllib3': "urllib3 not allowed",
            'ftplib': "FTP not allowed",
            'smtplib': "SMTP not allowed",
            'telnetlib': "Telnet not allowed",
        }
        
        # Forbidden patterns for sandbox escape
        self.sandbox_escape_patterns = [
            (r'/proc/', "Accessing /proc filesystem"),
            (r'/sys/', "Accessing /sys filesystem"),
            (r'/dev/', "Accessing /dev filesystem"),
            (r'os\.chroot', "chroot attempt"),
            (r'os\.setuid|os\.setgid', "Privilege manipulation"),
            (r'sys\._', "Accessing private sys attributes"),
            (r'__class__\.__bases__', "Class hierarchy manipulation"),
            (r'__subclasses__', "Subclass enumeration"),
            (r'__globals__', "Global namespace access"),
            (r'__code__', "Code object manipulation"),
            (r'os\.environ\[', "Direct environment manipulation"),
        ]
    
    def analyze_code(self, code: str, filename: str = "agent.py") -> CodeAnalysis:
        """Verify code complies with term-challenge rules."""
        issues = []
        warnings = []
        positives = []
        
        lines = code.split('\n')
        
        # === CHECK SDK PATTERNS ===
        
        sdk_signals = []
        for pattern, description in self.valid_sdk_patterns:
            if re.search(pattern, code):
                sdk_signals.append(description)
        
        # Determine SDK version
        has_term_sdk = any('term_sdk' in s for s in sdk_signals)
        has_argparse = any('argparse' in s for s in sdk_signals)
        
        if has_term_sdk:
            positives.append("Uses term_sdk (SDK 2.0 pattern)")
            # Check for required methods
            if 'run(self, ctx' not in code and 'solve(self, req' not in code:
                warnings.append("Missing run() or solve() method - required for term_sdk agents")
        elif has_argparse:
            positives.append("Uses argparse (SDK 3.0 pattern)")
            # Check for instruction handling
            if '--instruction' not in code:
                warnings.append("Missing --instruction argument - required for SDK 3.0")
        else:
            issues.append("No recognized SDK pattern found (need term_sdk or argparse+subprocess)")
        
        # === CHECK FORBIDDEN MODULES ===
        
        imported_modules = self._extract_imports(code)
        
        for module, reason in self.forbidden_network_modules.items():
            if module in imported_modules:
                issues.append(f"Forbidden module '{module}': {reason}")
        
        # === CHECK SANDBOX ESCAPE ATTEMPTS ===
        
        for pattern, description in self.sandbox_escape_patterns:
            matches = list(re.finditer(pattern, code))
            for match in matches:
                line_num = code[:match.start()].count('\n') + 1
                issues.append(f"Line {line_num}: Potential sandbox escape - {description}")
        
        # === CHECK AGENT STRUCTURE ===
        
        # Check for main guard
        if "__name__" in code and "__main__" in code:
            positives.append("Has __main__ guard")
        else:
            warnings.append("Missing if __name__ == '__main__' guard")
        
        # Check for proper class structure (if using OOP)
        classes = re.findall(r'class\s+(\w+)', code)
        if classes:
            for cls in classes:
                if 'Agent' in cls or 'agent' in cls.lower():
                    positives.append(f"Has agent class: {cls}")
        
        # Check for proper imports at top
        first_code_line = None
        for i, line in enumerate(lines):
            stripped = line.strip()
            if stripped and not stripped.startswith('#') and not stripped.startswith('"""'):
                first_code_line = i
                break
        
        if first_code_line:
            pre_code = '\n'.join(lines[:first_code_line + 10])
            if 'import' not in pre_code:
                warnings.append("No imports found near top of file")
        
        # === CHECK FOR COMMON ISSUES ===
        
        # Infinite loops without guards
        while_loops = re.findall(r'while\s+(True|1)\s*:', code)
        if while_loops:
            # Check if there's a step/iteration guard
            if 'ctx.step' not in code and 'step <' not in code and 'break' not in code:
                warnings.append("Infinite loop detected without apparent step limit guard")
        
        # Missing done() call
        if has_term_sdk and 'ctx.done()' not in code and '.done()' not in code:
            warnings.append("No ctx.done() call found - agent may never complete")
        
        # === DETERMINE VERDICT ===
        
        if issues:
            verdict = ReviewVerdict.REJECT
            confidence = min(0.95, 0.7 + len(issues) * 0.05)
        elif len(warnings) > 3:
            verdict = ReviewVerdict.NEEDS_DISCUSSION
            confidence = 0.65
        elif sdk_signals:
            verdict = ReviewVerdict.APPROVE
            confidence = 0.85
        else:
            verdict = ReviewVerdict.NEEDS_DISCUSSION
            confidence = 0.5
        
        return CodeAnalysis(
            issues=issues,
            warnings=warnings,
            positives=positives,
            verdict=verdict,
            confidence=confidence
        )
    
    def _extract_imports(self, code: str) -> Set[str]:
        """Extract all imported module names."""
        modules = set()
        
        # import x, y, z
        for match in re.finditer(r'^import\s+([\w\s,]+)', code, re.MULTILINE):
            for mod in match.group(1).split(','):
                mod = mod.strip().split()[0]  # Handle "import x as y"
                modules.add(mod.split('.')[0])
        
        # from x import y
        for match in re.finditer(r'^from\s+([\w\.]+)\s+import', code, re.MULTILINE):
            modules.add(match.group(1).split('.')[0])
        
        return modules
    
    def respond_to_discussion(
        self,
        code: str,
        conversation: List[ReviewMessage],
        my_analysis: CodeAnalysis
    ) -> ReviewMessage:
        """Respond to discussion."""
        other_messages = [m for m in conversation if m.agent_name != self.name]
        
        if not other_messages:
            content = self._format_initial_response(my_analysis)
            return self._create_message(content, my_analysis.verdict)
        
        last_msg = other_messages[-1]
        
        if self.llm_client:
            return self._llm_respond(code, conversation, my_analysis, last_msg)
        
        return self._rule_based_respond(my_analysis, last_msg)
    
    def _format_initial_response(self, analysis: CodeAnalysis) -> str:
        """Format initial analysis."""
        parts = ["## Rules Compliance Analysis\n"]
        
        if analysis.issues:
            parts.append("**🚨 Rule Violations:**")
            for issue in analysis.issues:
                parts.append(f"- {issue}")
            parts.append("")
        
        if analysis.warnings:
            parts.append("**⚠️ Compliance Concerns:**")
            for warning in analysis.warnings:
                parts.append(f"- {warning}")
            parts.append("")
        
        if analysis.positives:
            parts.append("**✅ Compliance Verified:**")
            for positive in analysis.positives:
                parts.append(f"- {positive}")
            parts.append("")
        
        verdict_emoji = {
            ReviewVerdict.APPROVE: "✅",
            ReviewVerdict.REJECT: "❌",
            ReviewVerdict.NEEDS_DISCUSSION: "🤔"
        }
        
        parts.append(f"**Verdict:** {verdict_emoji[analysis.verdict]} {analysis.verdict.value}")
        parts.append(f"**Confidence:** {analysis.confidence:.0%}")
        
        return "\n".join(parts)
    
    def _rule_based_respond(
        self,
        analysis: CodeAnalysis,
        last_msg: ReviewMessage
    ) -> ReviewMessage:
        """Generate rule-based response."""
        # Rule compliance is strict - don't easily change verdict
        if analysis.issues:
            content = (
                f"Responding to @{last_msg.agent_name}: While I appreciate the input, "
                f"the code has {len(analysis.issues)} rule violation(s) that must be fixed. "
                f"These are non-negotiable per AGENTS.md requirements."
            )
            return self._create_message(content, ReviewVerdict.REJECT, last_msg.agent_name)
        
        content = (
            f"@{last_msg.agent_name}: From a rules perspective, the code is compliant. "
            f"SDK patterns are correct and no forbidden modules detected."
        )
        
        return self._create_message(content, analysis.verdict, last_msg.agent_name)
    
    def _llm_respond(
        self,
        code: str,
        conversation: List[ReviewMessage],
        analysis: CodeAnalysis,
        last_msg: ReviewMessage
    ) -> ReviewMessage:
        """Generate LLM response."""
        conv_text = "\n\n".join([
            f"**{m.agent_name}** ({m.verdict.value if m.verdict else 'N/A'}):\n{m.content}"
            for m in conversation[-5:]
        ])
        
        prompt = f"""You are the Rules Compliance Verifier in a code review.
Your analysis found:
- Rule Violations: {analysis.issues}
- Warnings: {analysis.warnings}
- Compliant: {analysis.positives}

Recent conversation:
{conv_text}

Last message was from {last_msg.agent_name}. Respond:
1. Rule compliance is non-negotiable
2. Cite specific rules from AGENTS.md if relevant
3. Be firm but professional
4. End with verdict: APPROVE, REJECT, or NEEDS_DISCUSSION

Keep under 200 words."""

        try:
            response = self.llm_client.chat([
                {"role": "system", "content": self.get_system_prompt()},
                {"role": "user", "content": prompt}
            ])
            
            # Rule violations = always reject
            verdict = ReviewVerdict.REJECT if analysis.issues else analysis.verdict
            if not analysis.issues:
                if "APPROVE" in response.upper():
                    verdict = ReviewVerdict.APPROVE
                elif "NEEDS_DISCUSSION" in response.upper():
                    verdict = ReviewVerdict.NEEDS_DISCUSSION
            
            return self._create_message(response, verdict, last_msg.agent_name)
        except Exception:
            return self._rule_based_respond(analysis, last_msg)
    
    def get_system_prompt(self) -> str:
        return """You are a Rules Compliance Verifier for term-challenge.
You verify code against these rules from AGENTS.md:

1. Must use term_sdk (SDK 2.0) OR argparse+subprocess (SDK 3.0)
2. No forbidden network modules (socket, urllib direct access)
3. No sandbox escape attempts
4. Proper agent structure with required methods
5. Must call ctx.done() or have clear completion

You are strict but fair. Rule violations are non-negotiable.
Security and compliance override other considerations."""
