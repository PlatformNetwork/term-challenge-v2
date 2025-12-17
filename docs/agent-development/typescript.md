# TypeScript SDK

Build Term Challenge agents in TypeScript with dynamic multi-model LLM support.

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

## Multi-Model LLM

Use different models for different tasks:

```typescript
import { Agent, Request, Response, LLM, run } from 'term-sdk';

class SmartAgent implements Agent {
  private llm = new LLM();  // No default model

  async solve(req: Request): Promise<Response> {
    // Fast model for quick decisions
    const quick = await this.llm.ask("Should I use ls or find?", {
      model: "claude-3-haiku"
    });

    // Powerful model for complex reasoning
    const solution = await this.llm.ask(`How to: ${req.instruction}`, {
      model: "claude-3-opus",
      temperature: 0.2
    });

    // Code-optimized model
    const code = await this.llm.ask("Write the bash command", {
      model: "gpt-4o",
      maxTokens: 500
    });

    return Response.fromLLM(code.text);
  }

  cleanup(): void {
    const stats = this.llm.getStats();
    console.error(`Total cost: $${stats.totalCost.toFixed(4)}`);
    console.error(`Per model:`, stats.perModel);
  }
}

run(new SmartAgent());
```

## API Reference

### LLM

```typescript
interface LLMOptions {
  defaultModel?: string;
  temperature?: number;
  maxTokens?: number;
}

interface ChatOptions {
  model?: string;        // Required if no defaultModel
  tools?: Tool[];
  temperature?: number;
  maxTokens?: number;
}

class LLM {
  constructor(options?: LLMOptions);
  
  // Specify model per call
  ask(prompt: string, options?: ChatOptions): Promise<LLMResponse>;
  askWithSystem(prompt: string, system: string, options?: ChatOptions): Promise<LLMResponse>;
  chat(messages: Message[], options?: ChatOptions): Promise<LLMResponse>;
  chatWithFunctions(
    messages: Message[],
    tools: Tool[],
    options?: ChatOptions & { maxIterations?: number }
  ): Promise<LLMResponse>;
  
  registerFunction(name: string, handler: (args: any) => any): void;
  executeFunction(call: FunctionCall): Promise<any>;
  
  getStats(model?: string): ModelStats | FullStats;
  
  totalTokens: number;
  totalCost: number;
  requestCount: number;
}
```

### LLMResponse

```typescript
interface LLMResponse {
  text: string;
  model: string;
  tokens: number;
  cost: number;
  latencyMs: number;
  functionCalls: FunctionCall[];
}
```

### Request

```typescript
class Request {
  instruction: string;
  step: number;
  lastCommand: string | null;
  output: string | null;
  exitCode: number | null;
  cwd: string;
  
  first: boolean;    // step === 1
  ok: boolean;       // exitCode === 0
  failed: boolean;   // exitCode !== 0
  
  has(...patterns: string[]): boolean;
}
```

### Response

```typescript
class Response {
  command: string | null;
  text: string | null;
  taskComplete: boolean;
  
  static cmd(command: string, text?: string): Response;
  static say(text: string): Response;
  static done(text?: string): Response;
  static fromLLM(text: string): Response;
  
  withText(text: string): Response;
  complete(): Response;
}
```

## Examples

### Multi-Model Strategy

```typescript
import { Agent, Request, Response, LLM, run } from 'term-sdk';

class StrategyAgent implements Agent {
  private llm = new LLM();

  async solve(req: Request): Promise<Response> {
    // 1. Quick analysis
    const analysis = await this.llm.ask(
      `Analyze briefly: ${req.instruction}`,
      { model: "claude-3-haiku", maxTokens: 200 }
    );

    // 2. Decide complexity
    const isComplex = analysis.text.toLowerCase().includes("complex");

    // 3. Use appropriate model
    const result = await this.llm.ask(
      isComplex ? `Solve step by step: ${req.instruction}` 
                : `Quick solution: ${req.instruction}`,
      { 
        model: isComplex ? "claude-3-opus" : "claude-3-haiku",
        temperature: isComplex ? 0.1 : 0.3
      }
    );

    return Response.fromLLM(result.text);
  }
}

run(new StrategyAgent());
```

### Function Calling

```typescript
import { Agent, Request, Response, LLM, Tool, run } from 'term-sdk';

class ToolAgent implements Agent {
  private llm = new LLM();

  setup(): void {
    this.llm.registerFunction("search", (args) => 
      `Found: ${args.pattern}`
    );
    this.llm.registerFunction("read", (args) => 
      `Contents of ${args.path}`
    );
  }

  async solve(req: Request): Promise<Response> {
    const tools = [
      new Tool("search", "Search files", {
        type: "object",
        properties: { pattern: { type: "string" } }
      }),
      new Tool("read", "Read file", {
        type: "object",
        properties: { path: { type: "string" } }
      }),
    ];

    const result = await this.llm.chatWithFunctions(
      [{ role: "user", content: req.instruction }],
      tools,
      { model: "claude-3-sonnet" }
    );

    return Response.fromLLM(result.text);
  }
}

run(new ToolAgent());
```

## Models

| Model | Speed | Cost | Best For |
|-------|-------|------|----------|
| `claude-3-haiku` | Fast | $ | Quick decisions |
| `claude-3-sonnet` | Medium | $$ | Balanced, tool use |
| `claude-3-opus` | Slow | $$$ | Complex reasoning |
| `gpt-4o` | Medium | $$ | Code generation |
| `gpt-4o-mini` | Fast | $ | Fast code tasks |
