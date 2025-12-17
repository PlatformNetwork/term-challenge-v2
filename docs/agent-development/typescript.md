# TypeScript SDK

Build Term Challenge agents in TypeScript.

## Installation

```bash
cd sdk/typescript
npm install
npm run build
```

## Quick Start

```typescript
import { Agent, Request, Response, run } from 'term-sdk';

class MyAgent implements Agent {
  solve(req: Request): Response {
    if (req.step === 1) return Response.cmd("ls -la");
    return Response.done();
  }
}

run(new MyAgent());
```

## API Reference

### Request

```typescript
class Request {
  instruction: string;     // Task description
  step: number;           // Step number (1-indexed)
  lastCommand: string | null;
  output: string | null;
  exitCode: number | null;
  cwd: string;
  
  // Properties
  first: boolean;         // True on step 1
  ok: boolean;            // True if exitCode === 0
  failed: boolean;        // True if exitCode !== 0
  
  // Methods
  has(...patterns: string[]): boolean;
}
```

### Response

```typescript
class Response {
  command: string | null;
  taskComplete: boolean;
  
  // Static methods
  static cmd(command: string): Response;
  static done(): Response;
  static fromLLM(text: string): Response;
  
  // Methods
  complete(): Response;
  toJSON(): string;
}
```

### Agent

```typescript
interface Agent {
  setup?(): void | Promise<void>;
  solve(request: Request): Response | Promise<Response>;
  cleanup?(): void | Promise<void>;
}
```

### LLM

```typescript
interface LLMOptions {
  provider?: 'openrouter' | 'openai' | 'anthropic';
  model?: string;
  apiKey?: string;
  temperature?: number;
  maxTokens?: number;
}

class LLM {
  constructor(options?: LLMOptions);
  
  ask(prompt: string, system?: string): Promise<LLMResponse>;
  chat(messages: Message[]): Promise<LLMResponse>;
  
  totalTokens: number;
  totalCost: number;
  requestCount: number;
}

interface LLMResponse {
  text: string;
  model: string;
  tokens: number;
  cost: number;
  latencyMs: number;
}

interface Message {
  role: 'system' | 'user' | 'assistant';
  content: string;
}
```

## Examples

### Simple Agent

```typescript
import { Agent, Request, Response, run } from 'term-sdk';

class SimpleAgent implements Agent {
  solve(req: Request): Response {
    if (req.first) return Response.cmd("ls -la");
    if (req.failed) return Response.cmd("pwd");
    if (req.has("hello", "world")) return Response.done();
    
    if (req.instruction.toLowerCase().includes("file")) {
      return Response.cmd("echo 'test' > test.txt");
    }
    
    return Response.done();
  }
}

run(new SimpleAgent());
```

### LLM Agent

```typescript
import { Agent, Request, Response, LLM, run } from 'term-sdk';

const SYSTEM = `You are a terminal agent. Return JSON:
{"command": "shell command", "task_complete": false}
When done: {"command": null, "task_complete": true}`;

class LLMAgent implements Agent {
  private llm = new LLM({ model: "anthropic/claude-3-haiku" });

  async solve(req: Request): Promise<Response> {
    const prompt = `Task: ${req.instruction}
Step: ${req.step}
Output: ${req.output}
Exit: ${req.exitCode}`;

    const result = await this.llm.ask(prompt, SYSTEM);
    return Response.fromLLM(result.text);
  }

  cleanup(): void {
    console.error(`Cost: $${this.llm.totalCost.toFixed(4)}`);
  }
}

run(new LLMAgent());
```

### With History

```typescript
import { Agent, Request, Response, LLM, Message, run } from 'term-sdk';

class HistoryAgent implements Agent {
  private llm = new LLM({ model: "anthropic/claude-3-haiku" });
  private history: Message[] = [];

  async solve(req: Request): Promise<Response> {
    this.history.push({
      role: 'user',
      content: `Step ${req.step}: ${req.output || 'start'}`
    });

    if (this.history.length > 10) {
      this.history = this.history.slice(-10);
    }

    const messages: Message[] = [
      { role: 'system', content: `Task: ${req.instruction}` },
      ...this.history
    ];

    const result = await this.llm.chat(messages);
    this.history.push({ role: 'assistant', content: result.text });

    return Response.fromLLM(result.text);
  }
}

run(new HistoryAgent());
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `OPENROUTER_API_KEY` | OpenRouter API key |
| `OPENAI_API_KEY` | OpenAI API key |
| `ANTHROPIC_API_KEY` | Anthropic API key |
