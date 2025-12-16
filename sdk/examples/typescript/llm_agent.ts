#!/usr/bin/env -S npx tsx
/**
 * LLM Agent - Professional terminal agent powered by LLM.
 * 
 * Usage:
 *   export OPENROUTER_API_KEY="sk-or-..."
 *   npx tsx llm_agent.ts
 * 
 *   # Or with term CLI:
 *   term bench agent -a ./llm_agent.ts -t ~/.cache/term-challenge/datasets/hello-world
 */

import { Agent, AgentResponse, Command, Harness, LLMClient, Message, parseJsonResponse } from '../../typescript/src/index.ts';

// =============================================================================
// System Prompt
// =============================================================================

const SYSTEM_PROMPT = `You are an expert terminal agent. Your goal is to complete tasks using only terminal commands.

## Response Format

Respond with JSON only:
\`\`\`json
{
  "analysis": "What you observe in the terminal",
  "plan": "Your step-by-step plan",
  "commands": [
    {"keystrokes": "your_command\\n", "duration": 1.0}
  ],
  "task_complete": false
}
\`\`\`

## Rules

1. **Commands**: Include \`\\n\` at the end to execute commands
2. **Duration**: Use longer durations (5-30s) for slow operations
3. **Verification**: Always verify your work before setting task_complete=true
4. **One step at a time**: Execute one logical operation per response

## Special Keys

- \`\\n\` = Enter
- \`\\t\` = Tab
- \`\\x03\` = Ctrl+C
- \`\\x04\` = Ctrl+D`;

// =============================================================================
// LLM Agent
// =============================================================================

class LLMAgent extends Agent {
    private client: LLMClient | null = null;
    private conversationHistory: Message[] = [];

    async setup(): Promise<void> {
        const provider = (process.env.LLM_PROVIDER ?? 'openrouter') as 'openrouter' | 'chutes';
        const model = process.env.LLM_MODEL;
        const budget = parseFloat(process.env.LLM_BUDGET ?? '10.0');

        this.client = new LLMClient({
            provider,
            model,
            budget
        });

        console.error(`[LLMAgent] Initialized: ${provider}/${this.client.model}`);
    }

    async step(instruction: string, screen: string, step: number): Promise<AgentResponse> {
        if (!this.client) {
            return AgentResponse.error('Client not initialized');
        }

        // Build prompt
        const userMessage = `## Task
${instruction}

## Terminal (Step ${step})
\`\`\`
${screen}
\`\`\`

Analyze the terminal and respond with JSON for your next action.`;

        // Build messages
        const messages: Message[] = [
            { role: 'system', content: SYSTEM_PROMPT },
            ...this.conversationHistory,
            { role: 'user', content: userMessage }
        ];

        try {
            // Call LLM
            const response = await this.client.chat(messages, {
                temperature: 0.7,
                maxTokens: 2048
            });

            console.error(
                `[LLMAgent] Step ${step}: ${response.totalTokens} tokens, ` +
                `$${response.cost.toFixed(4)} (total: $${this.client.totalCost.toFixed(4)})`
            );

            // Update history (keep last 10 exchanges)
            this.conversationHistory.push({ role: 'user', content: userMessage });
            this.conversationHistory.push({ role: 'assistant', content: response.content });
            if (this.conversationHistory.length > 20) {
                this.conversationHistory = this.conversationHistory.slice(-20);
            }

            // Parse response
            return this.parseResponse(response.content);

        } catch (error) {
            console.error(`[LLMAgent] Error: ${error}`);
            return AgentResponse.error(String(error));
        }
    }

    async cleanup(): Promise<void> {
        if (this.client) {
            console.error(
                `[LLMAgent] Session complete: ${this.client.requestCount} requests, ` +
                `$${this.client.totalCost.toFixed(4)} total`
            );
        }
    }

    private parseResponse(content: string): AgentResponse {
        try {
            const data = parseJsonResponse(content);

            // Parse commands
            const commands: Command[] = (data.commands ?? []).map((cmd: any) => {
                if (typeof cmd === 'object') {
                    return new Command(cmd.keystrokes ?? '', cmd.duration ?? 1.0);
                }
                return new Command(String(cmd));
            });

            return new AgentResponse({
                analysis: data.analysis ?? '',
                plan: data.plan ?? '',
                commands,
                taskComplete: Boolean(data.task_complete)
            });

        } catch (error) {
            console.error(`[LLMAgent] Parse error: ${error}`);
            return new AgentResponse({
                analysis: `Failed to parse response: ${error}`,
                plan: content.slice(0, 500),
                commands: [],
                taskComplete: false
            });
        }
    }
}

// =============================================================================
// Main
// =============================================================================

const agent = new LLMAgent();
new Harness(agent).run().catch(error => {
    console.error(`Fatal error: ${error}`);
    process.exit(1);
});
