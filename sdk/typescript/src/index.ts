/**
 * Term SDK for TypeScript
 * 
 * Build agents for Term Challenge.
 * 
 * @example Quick Start
 * ```typescript
 * import { Agent, Request, Response, run } from 'term-sdk';
 * 
 * class MyAgent implements Agent {
 *   solve(req: Request): Response {
 *     if (req.step === 1) return Response.cmd("ls -la");
 *     if (req.has("hello")) return Response.done();
 *     return Response.cmd("echo hello");
 *   }
 * }
 * 
 * run(new MyAgent());
 * ```
 * 
 * @example With LLM
 * ```typescript
 * import { Agent, Request, Response, LLM, run } from 'term-sdk';
 * 
 * class LLMAgent implements Agent {
 *   private llm = new LLM({ model: "anthropic/claude-3-haiku" });
 * 
 *   async solve(req: Request): Promise<Response> {
 *     const result = await this.llm.ask(`Task: ${req.instruction}`);
 *     return Response.fromLLM(result.text);
 *   }
 * }
 * 
 * run(new LLMAgent());
 * ```
 */

import * as readline from 'readline';

// ============================================================================
// Types
// ============================================================================

/**
 * Request from the harness.
 */
export interface RequestData {
  instruction: string;
  step: number;
  last_command: string | null;
  output: string | null;
  exit_code: number | null;
  cwd: string;
}

/**
 * Request wrapper with helper methods.
 */
export class Request {
  readonly instruction: string;
  readonly step: number;
  readonly lastCommand: string | null;
  readonly output: string | null;
  readonly exitCode: number | null;
  readonly cwd: string;

  constructor(data: RequestData) {
    this.instruction = data.instruction;
    this.step = data.step;
    this.lastCommand = data.last_command;
    this.output = data.output;
    this.exitCode = data.exit_code;
    this.cwd = data.cwd || "/app";
  }

  static parse(json: string): Request {
    return new Request(JSON.parse(json));
  }

  /** True if this is the first step */
  get first(): boolean {
    return this.step === 1;
  }

  /** True if last command succeeded */
  get ok(): boolean {
    return this.exitCode === 0;
  }

  /** True if last command failed */
  get failed(): boolean {
    return this.exitCode !== null && this.exitCode !== 0;
  }

  /** Check if output contains any pattern (case-insensitive) */
  has(...patterns: string[]): boolean {
    if (!this.output) return false;
    const lower = this.output.toLowerCase();
    return patterns.some(p => lower.includes(p.toLowerCase()));
  }
}

/**
 * Response to the harness.
 */
export class Response {
  command: string | null;
  taskComplete: boolean;

  constructor(command: string | null = null, taskComplete = false) {
    this.command = command;
    this.taskComplete = taskComplete;
  }

  /** Create response with command */
  static cmd(command: string): Response {
    return new Response(command, false);
  }

  /** Create response marking task complete */
  static done(): Response {
    return new Response(null, true);
  }

  /** Mark task as complete */
  complete(): Response {
    this.taskComplete = true;
    return this;
  }

  /** Convert to JSON string */
  toJSON(): string {
    return JSON.stringify({
      command: this.command,
      task_complete: this.taskComplete,
    });
  }

  /** Parse response from LLM output */
  static fromLLM(text: string): Response {
    text = text.trim();

    // Remove markdown code blocks
    const codeMatch = text.match(/```(?:json)?\s*(\{[\s\S]*?\})\s*```/);
    if (codeMatch) {
      text = codeMatch[1];
    }

    // Find JSON object
    const start = text.indexOf('{');
    const end = text.lastIndexOf('}');

    if (start >= 0 && end > start) {
      try {
        const data = JSON.parse(text.slice(start, end + 1));
        return new Response(
          data.command ?? null,
          data.task_complete ?? false
        );
      } catch {
        // Invalid JSON
      }
    }

    return Response.done();
  }
}

// ============================================================================
// Agent
// ============================================================================

/**
 * Agent interface.
 */
export interface Agent {
  /** Initialize (optional) */
  setup?(): void | Promise<void>;
  
  /** Process request and return response */
  solve(request: Request): Response | Promise<Response>;
  
  /** Cleanup (optional) */
  cleanup?(): void | Promise<void>;
}

// ============================================================================
// Runner
// ============================================================================

function log(msg: string): void {
  console.error(`[agent] ${msg}`);
}

/**
 * Run an agent in the Term Challenge harness.
 */
export async function run(agent: Agent): Promise<void> {
  try {
    // Setup
    if (agent.setup) {
      await agent.setup();
    }

    // Read input
    const input = await readStdin();
    if (!input) {
      log("No input received");
      console.log(Response.done().toJSON());
      return;
    }

    // Parse request
    let request: Request;
    try {
      request = Request.parse(input);
    } catch (e) {
      log(`Invalid JSON: ${e}`);
      console.log(Response.done().toJSON());
      return;
    }

    log(`Step ${request.step}: ${request.instruction.slice(0, 50)}...`);

    // Solve
    const response = await agent.solve(request);

    // Output
    console.log(response.toJSON());

    // Cleanup
    if (agent.cleanup) {
      await agent.cleanup();
    }
  } catch (e) {
    log(`Error: ${e}`);
    console.log(Response.done().toJSON());
  }
}

async function readStdin(): Promise<string> {
  return new Promise((resolve) => {
    let data = '';
    process.stdin.setEncoding('utf8');
    process.stdin.on('data', (chunk) => { data += chunk; });
    process.stdin.on('end', () => resolve(data.trim()));
    
    // Handle no input
    setTimeout(() => {
      if (!data) resolve('');
    }, 100);
  });
}

// ============================================================================
// LLM Client
// ============================================================================

export type Provider = 'openrouter' | 'openai' | 'anthropic';

export interface LLMOptions {
  provider?: Provider;
  model?: string;
  apiKey?: string;
  temperature?: number;
  maxTokens?: number;
  timeout?: number;
}

export interface LLMResponse {
  text: string;
  model: string;
  tokens: number;
  cost: number;
  latencyMs: number;
}

export interface Message {
  role: 'system' | 'user' | 'assistant';
  content: string;
}

const PROVIDER_CONFIG = {
  openrouter: {
    url: 'https://openrouter.ai/api/v1/chat/completions',
    envKey: 'OPENROUTER_API_KEY',
  },
  openai: {
    url: 'https://api.openai.com/v1/chat/completions',
    envKey: 'OPENAI_API_KEY',
  },
  anthropic: {
    url: 'https://api.anthropic.com/v1/messages',
    envKey: 'ANTHROPIC_API_KEY',
  },
};

const PRICING: Record<string, [number, number]> = {
  'anthropic/claude-3-haiku': [0.25, 1.25],
  'anthropic/claude-3-sonnet': [3.0, 15.0],
  'anthropic/claude-3-opus': [15.0, 75.0],
  'openai/gpt-4o': [5.0, 15.0],
  'openai/gpt-4o-mini': [0.15, 0.6],
  'gpt-4o': [5.0, 15.0],
  'gpt-4o-mini': [0.15, 0.6],
};

/**
 * LLM client for multiple providers.
 */
export class LLM {
  private provider: Provider;
  private model: string;
  private apiKey: string;
  private temperature: number;
  private maxTokens: number;
  private timeout: number;

  totalTokens = 0;
  totalCost = 0;
  requestCount = 0;

  constructor(options: LLMOptions = {}) {
    this.provider = options.provider || 'openrouter';
    this.model = options.model || 'anthropic/claude-3-haiku';
    this.temperature = options.temperature ?? 0.3;
    this.maxTokens = options.maxTokens ?? 1024;
    this.timeout = options.timeout ?? 60000;

    const config = PROVIDER_CONFIG[this.provider];
    this.apiKey = options.apiKey || process.env[config.envKey] || '';
    
    if (!this.apiKey) {
      console.error(`[llm] Warning: ${config.envKey} not set`);
    }
  }

  /** Ask a simple question */
  async ask(prompt: string, system?: string): Promise<LLMResponse> {
    const messages: Message[] = [];
    if (system) messages.push({ role: 'system', content: system });
    messages.push({ role: 'user', content: prompt });
    return this.chat(messages);
  }

  /** Chat with messages */
  async chat(messages: Message[]): Promise<LLMResponse> {
    const start = Date.now();

    const response = this.provider === 'anthropic'
      ? await this.chatAnthropic(messages)
      : await this.chatOpenAI(messages);

    response.latencyMs = Date.now() - start;

    this.totalTokens += response.tokens;
    this.totalCost += response.cost;
    this.requestCount++;

    console.error(`[llm] ${response.model}: ${response.tokens} tokens, $${response.cost.toFixed(4)}, ${response.latencyMs}ms`);

    return response;
  }

  private async chatOpenAI(messages: Message[]): Promise<LLMResponse> {
    const config = PROVIDER_CONFIG[this.provider];

    const response = await fetch(config.url, {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${this.apiKey}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        model: this.model,
        messages,
        temperature: this.temperature,
        max_tokens: this.maxTokens,
      }),
      signal: AbortSignal.timeout(this.timeout),
    });

    if (!response.ok) {
      throw new Error(`API error: ${response.status}`);
    }

    const data = await response.json() as any;
    const text = data.choices?.[0]?.message?.content || '';
    const promptTokens = data.usage?.prompt_tokens || 0;
    const completionTokens = data.usage?.completion_tokens || 0;

    return {
      text,
      model: this.model,
      tokens: promptTokens + completionTokens,
      cost: this.calculateCost(promptTokens, completionTokens),
      latencyMs: 0,
    };
  }

  private async chatAnthropic(messages: Message[]): Promise<LLMResponse> {
    const config = PROVIDER_CONFIG.anthropic;

    let system: string | undefined;
    const userMessages = messages.filter(m => {
      if (m.role === 'system') {
        system = m.content;
        return false;
      }
      return true;
    });

    const body: any = {
      model: this.model,
      messages: userMessages,
      temperature: this.temperature,
      max_tokens: this.maxTokens,
    };
    if (system) body.system = system;

    const response = await fetch(config.url, {
      method: 'POST',
      headers: {
        'x-api-key': this.apiKey,
        'Content-Type': 'application/json',
        'anthropic-version': '2023-06-01',
      },
      body: JSON.stringify(body),
      signal: AbortSignal.timeout(this.timeout),
    });

    if (!response.ok) {
      throw new Error(`API error: ${response.status}`);
    }

    const data = await response.json() as any;
    const text = data.content?.[0]?.text || '';
    const promptTokens = data.usage?.input_tokens || 0;
    const completionTokens = data.usage?.output_tokens || 0;

    return {
      text,
      model: this.model,
      tokens: promptTokens + completionTokens,
      cost: this.calculateCost(promptTokens, completionTokens),
      latencyMs: 0,
    };
  }

  private calculateCost(promptTokens: number, completionTokens: number): number {
    const [inputPrice, outputPrice] = PRICING[this.model] || [0.5, 1.5];
    return (promptTokens * inputPrice + completionTokens * outputPrice) / 1_000_000;
  }
}
