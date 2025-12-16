/**
 * Term Challenge SDK - TypeScript/JavaScript
 * 
 * Professional framework for building terminal agents.
 * 
 * @example
 * ```typescript
 * import { Agent, AgentResponse, Command, Harness } from 'term-sdk';
 * 
 * class MyAgent extends Agent {
 *   async step(instruction: string, screen: string, step: number): Promise<AgentResponse> {
 *     return new AgentResponse({
 *       analysis: "Terminal shows prompt",
 *       plan: "Execute ls command",
 *       commands: [new Command("ls -la\n")],
 *       taskComplete: false
 *     });
 *   }
 * }
 * 
 * const agent = new MyAgent();
 * new Harness(agent).run();
 * ```
 */

import * as readline from 'readline';

// =============================================================================
// Types
// =============================================================================

/**
 * A command to send to the terminal.
 */
export class Command {
    /** The exact text to send (include \n to execute) */
    keystrokes: string;
    /** Seconds to wait after sending (default 1.0) */
    duration: number;

    constructor(keystrokes: string, duration: number = 1.0) {
        this.keystrokes = keystrokes;
        this.duration = duration;
    }

    toJSON(): object {
        return {
            keystrokes: this.keystrokes,
            duration: this.duration
        };
    }
}

/**
 * Request from harness to agent.
 */
export interface AgentRequest {
    instruction: string;
    screen: string;
    step: number;
}

/**
 * Response from agent to harness.
 */
export class AgentResponse {
    analysis: string;
    plan: string;
    commands: Command[];
    taskComplete: boolean;

    constructor(options: {
        analysis?: string;
        plan?: string;
        commands?: Command[];
        taskComplete?: boolean;
    } = {}) {
        this.analysis = options.analysis ?? '';
        this.plan = options.plan ?? '';
        this.commands = options.commands ?? [];
        this.taskComplete = options.taskComplete ?? false;
    }

    toJSON(): object {
        return {
            analysis: this.analysis,
            plan: this.plan,
            commands: this.commands.map(c => c.toJSON()),
            task_complete: this.taskComplete
        };
    }

    static error(message: string): AgentResponse {
        return new AgentResponse({
            analysis: `Error: ${message}`,
            plan: 'Cannot continue due to error',
            commands: [],
            taskComplete: false
        });
    }
}

// =============================================================================
// Agent Base Class
// =============================================================================

/**
 * Base class for Term Challenge agents.
 * 
 * Subclass this and implement the `step` method to create your agent.
 * 
 * @example
 * ```typescript
 * class MyAgent extends Agent {
 *   private client: LLMClient;
 * 
 *   async setup(): Promise<void> {
 *     this.client = new LLMClient({ provider: 'openrouter' });
 *   }
 * 
 *   async step(instruction: string, screen: string, step: number): Promise<AgentResponse> {
 *     const response = await this.client.chat([
 *       { role: 'user', content: `Task: ${instruction}\n\nTerminal:\n${screen}` }
 *     ]);
 *     return this.parseResponse(response.content);
 *   }
 * 
 *   async cleanup(): Promise<void> {
 *     // Clean up resources
 *   }
 * }
 * ```
 */
export abstract class Agent {
    /**
     * Initialize the agent. Override to set up resources.
     */
    async setup(): Promise<void> {
        // Default: no-op
    }

    /**
     * Process one step of the task.
     * 
     * @param instruction - The task instruction/goal.
     * @param screen - Current terminal screen content.
     * @param step - Current step number (1-indexed).
     * @returns AgentResponse with analysis, plan, commands, and taskComplete flag.
     */
    abstract step(instruction: string, screen: string, step: number): Promise<AgentResponse>;

    /**
     * Clean up resources. Override to release resources.
     */
    async cleanup(): Promise<void> {
        // Default: no-op
    }
}

// =============================================================================
// Harness
// =============================================================================

/**
 * Runs an agent in the Term Challenge harness.
 * 
 * The harness handles:
 * - Reading requests from stdin
 * - Calling the agent's step method
 * - Writing responses to stdout
 * - Error handling and logging
 * 
 * @example
 * ```typescript
 * const agent = new MyAgent();
 * const harness = new Harness(agent);
 * harness.run();
 * ```
 */
export class Harness {
    private agent: Agent;
    private running: boolean = false;
    private rl: readline.Interface | null = null;

    constructor(agent: Agent) {
        this.agent = agent;
    }

    /**
     * Run the agent loop (blocking).
     * 
     * This is the main entry point. Call this from your script.
     */
    async run(): Promise<void> {
        this.running = true;

        try {
            // Setup
            this.log('Setting up agent...');
            await this.agent.setup();
            this.log('Agent ready');

            // Run loop
            await this.processLoop();
        } catch (error) {
            this.log(`Fatal error: ${error}`);
            throw error;
        } finally {
            // Cleanup
            try {
                await this.agent.cleanup();
            } catch (error) {
                this.log(`Cleanup error: ${error}`);
            }
            this.close();
        }
    }

    /**
     * Stop the agent loop.
     */
    stop(): void {
        this.running = false;
        this.close();
    }

    private close(): void {
        if (this.rl) {
            this.rl.close();
            this.rl = null;
        }
    }

    private log(message: string): void {
        console.error(`[term-sdk] ${message}`);
    }

    private sendResponse(response: AgentResponse): void {
        try {
            console.log(JSON.stringify(response.toJSON()));
        } catch (error) {
            this.log(`Failed to send response: ${error}`);
            console.log(JSON.stringify({
                analysis: `Error: ${error}`,
                plan: '',
                commands: [],
                task_complete: false
            }));
        }
    }

    private async processLoop(): Promise<void> {
        // Use synchronous-style line reading to properly block
        const rl = readline.createInterface({
            input: process.stdin,
            terminal: false
        });

        for await (const line of rl) {
            if (!this.running) break;

            const trimmed = line.trim();
            if (!trimmed) continue;

            try {
                const response = await this.processRequest(trimmed);
                this.sendResponse(response);
                
                // Flush stdout to ensure response is sent immediately
                if (process.stdout.write) {
                    await new Promise<void>(resolve => {
                        if (!process.stdout.write('')) {
                            process.stdout.once('drain', resolve);
                        } else {
                            resolve();
                        }
                    });
                }
            } catch (error) {
                this.log(`Error processing request: ${error}`);
                this.sendResponse(AgentResponse.error(String(error)));
            }
        }
    }

    private async processRequest(line: string): Promise<AgentResponse> {
        // Parse request
        let request: AgentRequest;
        try {
            request = JSON.parse(line) as AgentRequest;
        } catch (error) {
            return AgentResponse.error(`Invalid JSON: ${error}`);
        }

        const { instruction, screen, step } = request;

        // Call agent
        return await this.agent.step(instruction, screen, step);
    }
}

// =============================================================================
// LLM Client
// =============================================================================

export type Provider = 'openrouter' | 'chutes' | 'openai' | 'anthropic' | 'custom';

export interface Message {
    role: 'system' | 'user' | 'assistant';
    content: string;
}

export interface ChatResponse {
    content: string;
    model: string;
    promptTokens: number;
    completionTokens: number;
    totalTokens: number;
    cost: number;
    latencyMs: number;
}

export interface LLMClientOptions {
    provider?: Provider;
    apiKey?: string;
    model?: string;
    baseUrl?: string;
    budget?: number;
    timeout?: number;
}

const PROVIDER_CONFIG: Record<string, { baseUrl: string; envKey: string; defaultModel: string }> = {
    openrouter: {
        baseUrl: 'https://openrouter.ai/api/v1',
        envKey: 'OPENROUTER_API_KEY',
        defaultModel: 'anthropic/claude-3-haiku'
    },
    chutes: {
        baseUrl: 'https://llm.chutes.ai/v1',
        envKey: 'CHUTES_API_KEY',
        defaultModel: 'Qwen/Qwen3-32B'
    },
    openai: {
        baseUrl: 'https://api.openai.com/v1',
        envKey: 'OPENAI_API_KEY',
        defaultModel: 'gpt-4o-mini'
    },
    anthropic: {
        baseUrl: 'https://api.anthropic.com/v1',
        envKey: 'ANTHROPIC_API_KEY',
        defaultModel: 'claude-3-haiku-20240307'
    }
};

const MODEL_PRICING: Record<string, [number, number]> = {
    'anthropic/claude-3-haiku': [0.25, 1.25],
    'anthropic/claude-3-sonnet': [3.0, 15.0],
    'anthropic/claude-sonnet-4': [3.0, 15.0],
    'openai/gpt-4o-mini': [0.15, 0.60],
    'gpt-4o-mini': [0.15, 0.60],
    'Qwen/Qwen3-32B': [0.10, 0.30]
};

/**
 * Multi-provider LLM client with cost tracking.
 */
export class LLMClient {
    private provider: Provider;
    private apiKey: string;
    private baseUrl: string;
    model: string;
    private timeout: number;
    private budget: number | null;
    
    totalCost: number = 0;
    totalTokens: number = 0;
    requestCount: number = 0;

    constructor(options: LLMClientOptions = {}) {
        this.provider = options.provider ?? 'openrouter';
        
        const config = PROVIDER_CONFIG[this.provider] ?? PROVIDER_CONFIG.openrouter;
        
        this.baseUrl = options.baseUrl ?? config.baseUrl;
        this.model = options.model ?? config.defaultModel;
        this.timeout = options.timeout ?? 300000;
        this.budget = options.budget ?? null;
        
        // Get API key
        this.apiKey = options.apiKey ?? process.env[config.envKey] ?? process.env.LLM_API_KEY ?? '';
        
        if (!this.apiKey) {
            throw new Error(`API key required. Set ${config.envKey} or pass apiKey option.`);
        }
    }

    /**
     * Send a chat completion request.
     */
    async chat(
        messages: Message[],
        options: {
            model?: string;
            temperature?: number;
            maxTokens?: number;
        } = {}
    ): Promise<ChatResponse> {
        // Check budget
        if (this.budget !== null && this.totalCost >= this.budget) {
            throw new Error(`Over budget: $${this.totalCost.toFixed(4)} >= $${this.budget.toFixed(4)}`);
        }

        const model = options.model ?? this.model;
        const url = `${this.baseUrl}/chat/completions`;

        const start = Date.now();

        const response = await fetch(url, {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${this.apiKey}`,
                'Content-Type': 'application/json',
                'HTTP-Referer': 'https://term-challenge.ai'
            },
            body: JSON.stringify({
                model,
                messages,
                temperature: options.temperature ?? 0.7,
                max_tokens: options.maxTokens ?? 4096
            }),
            signal: AbortSignal.timeout(this.timeout)
        });

        if (!response.ok) {
            throw new Error(`API error: ${response.status} ${response.statusText}`);
        }

        const result = await response.json() as any;
        const latencyMs = Date.now() - start;

        // Parse response
        let content = result.choices[0].message.content;
        
        // Remove <think> blocks (Qwen)
        content = content.replace(/<think>[\s\S]*?<\/think>/g, '').trim();

        // Get usage
        const usage = result.usage ?? {};
        const promptTokens = usage.prompt_tokens ?? 0;
        const completionTokens = usage.completion_tokens ?? 0;
        const totalTokens = usage.total_tokens ?? (promptTokens + completionTokens);

        // Calculate cost
        const pricing = MODEL_PRICING[model] ?? [0.5, 1.5];
        const cost = (promptTokens / 1_000_000) * pricing[0] + (completionTokens / 1_000_000) * pricing[1];

        // Track
        this.totalCost += cost;
        this.totalTokens += totalTokens;
        this.requestCount++;

        return {
            content,
            model,
            promptTokens,
            completionTokens,
            totalTokens,
            cost,
            latencyMs
        };
    }
}

// =============================================================================
// JSON Parser Utilities
// =============================================================================

/**
 * Parse JSON from LLM response with fallback regex parsing.
 * Handles malformed JSON that LLMs sometimes produce.
 */
export function parseJsonResponse(content: string): Record<string, any> {
    // Find JSON in response
    const start = content.indexOf('{');
    const end = content.lastIndexOf('}');

    if (start < 0 || end <= start) {
        throw new Error('No JSON found in response');
    }

    const jsonStr = content.slice(start, end + 1);

    // Try standard JSON parse first
    try {
        return JSON.parse(jsonStr);
    } catch {
        // Fallback: extract fields using regex
        const analysisMatch = jsonStr.match(/"analysis"\s*:\s*"((?:[^"\\]|\\.)*)"/);
        const planMatch = jsonStr.match(/"plan"\s*:\s*"((?:[^"\\]|\\.)*)"/);
        const taskCompleteMatch = jsonStr.match(/"task_complete"\s*:\s*(true|false)/);
        const commandsMatch = jsonStr.match(/"commands"\s*:\s*\[([\s\S]*?)\]/);

        const result: Record<string, any> = {
            analysis: analysisMatch ? unescapeString(analysisMatch[1]) : '',
            plan: planMatch ? unescapeString(planMatch[1]) : '',
            task_complete: taskCompleteMatch ? taskCompleteMatch[1] === 'true' : false,
            commands: []
        };

        // Parse commands
        if (commandsMatch) {
            const cmdRegex = /\{\s*"keystrokes"\s*:\s*"((?:[^"\\]|\\.)*)"\s*(?:,\s*"duration"\s*:\s*([\d.]+))?\s*\}/g;
            let cmdMatch;
            while ((cmdMatch = cmdRegex.exec(commandsMatch[1])) !== null) {
                result.commands.push({
                    keystrokes: unescapeString(cmdMatch[1]),
                    duration: cmdMatch[2] ? parseFloat(cmdMatch[2]) : 1.0
                });
            }
        }

        return result;
    }
}

function unescapeString(s: string): string {
    return s
        .replace(/\\n/g, '\n')
        .replace(/\\t/g, '\t')
        .replace(/\\r/g, '\r')
        .replace(/\\"/g, '"')
        .replace(/\\\\/g, '\\');
}

// =============================================================================
// Convenience Function
// =============================================================================

/**
 * Run an agent in the harness.
 * 
 * @example
 * ```typescript
 * run(new MyAgent());
 * ```
 */
export function run(agent: Agent): void {
    new Harness(agent).run().catch(error => {
        console.error(`Fatal error: ${error}`);
        process.exit(1);
    });
}

// =============================================================================
// Exports
// =============================================================================

export default {
    Agent,
    AgentResponse,
    Command,
    Harness,
    LLMClient,
    run
};
