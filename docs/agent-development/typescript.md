# TypeScript/JavaScript Agent Development

Complete guide for building Term Challenge agents in TypeScript or JavaScript.

## Installation

Install from the git repository:

```bash
# Clone the repository
git clone https://github.com/PlatformNetwork/term-challenge.git
cd term-challenge/sdk/typescript

# Install dependencies and build
npm install
npm run build

# Link for local development
npm link
```

Then in your project:

```bash
npm link term-sdk
```

Or copy the SDK directly into your project:

```bash
cp -r term-challenge/sdk/typescript ./term-sdk
npm install ./term-sdk
```

## SDK Overview

```typescript
import {
  // Core harness types
  Agent,           // Base class for agents
  AgentRequest,    // Request from harness
  AgentResponse,   // Response to harness
  Command,         // Terminal command
  Harness,         // Agent runner
  run,             // Convenience function
  
  // LLM client
  LLMClient,       // Multi-provider LLM client
  Provider,        // Provider type
  Message,         // Chat message
  ChatResponse,    // LLM response
  
  // Utilities
  parseJsonResponse  // Parse JSON from LLM output
} from 'term-sdk';
```

## Basic Agent Structure

### TypeScript

```typescript
import { Agent, AgentResponse, Command, run } from 'term-sdk';

class MyAgent extends Agent {
  /**
   * Initialize resources (optional).
   */
  async setup(): Promise<void> {
    // Setup code here
  }

  /**
   * Process one step of the task.
   * 
   * @param instruction - The task goal/description
   * @param screen - Current terminal content
   * @param step - Step number (1-indexed)
   */
  async step(instruction: string, screen: string, step: number): Promise<AgentResponse> {
    return new AgentResponse({
      analysis: "What I observe...",
      plan: "What I'll do...",
      commands: [new Command("ls -la\n", 0.5)],
      taskComplete: false
    });
  }

  /**
   * Clean up resources (optional).
   */
  async cleanup(): Promise<void> {
    // Cleanup code here
  }
}

run(new MyAgent());
```

### JavaScript (ESM)

```javascript
import { Agent, AgentResponse, Command, run } from 'term-sdk';

class MyAgent extends Agent {
  async setup() {
    // Setup code here
  }

  async step(instruction, screen, step) {
    return new AgentResponse({
      analysis: "What I observe...",
      plan: "What I'll do...",
      commands: [new Command("ls -la\n", 0.5)],
      taskComplete: false
    });
  }

  async cleanup() {
    // Cleanup code here
  }
}

run(new MyAgent());
```

### JavaScript (CommonJS)

```javascript
const { Agent, AgentResponse, Command, run } = require('term-sdk');

class MyAgent extends Agent {
  async step(instruction, screen, step) {
    return new AgentResponse({
      analysis: "What I observe...",
      plan: "What I'll do...",
      commands: [new Command("ls -la\n", 0.5)],
      taskComplete: false
    });
  }
}

run(new MyAgent());
```

## Core Types

### Command

```typescript
import { Command } from 'term-sdk';

// Basic command with Enter
const cmd = new Command("ls -la\n");

// Command with custom duration
const cmd = new Command("npm install\n", 10.0);

// Special keys (tmux-style)
const ctrl_c = new Command("C-c", 0.1);
const tab = new Command("Tab", 0.1);
const escape = new Command("Escape", 0.1);
```

### AgentResponse

```typescript
import { AgentResponse, Command } from 'term-sdk';

const response = new AgentResponse({
  analysis: "Terminal shows empty directory",
  plan: "Create the requested file",
  commands: [
    new Command("echo 'Hello' > hello.txt\n", 0.3),
    new Command("cat hello.txt\n", 0.3)
  ],
  taskComplete: false
});

// Create error response
const errorResponse = AgentResponse.error("Something went wrong");
```

### AgentRequest Interface

```typescript
interface AgentRequest {
  instruction: string;  // Task description
  screen: string;       // Terminal content
  step: number;         // Step number
}
```

## LLM Integration

### Basic LLM Agent

```typescript
import { Agent, AgentResponse, Command, run, LLMClient, parseJsonResponse } from 'term-sdk';

class LLMAgent extends Agent {
  private client!: LLMClient;

  async setup(): Promise<void> {
    // Uses OPENROUTER_API_KEY env var
    this.client = new LLMClient({ provider: 'openrouter' });
  }

  async step(instruction: string, screen: string, step: number): Promise<AgentResponse> {
    const prompt = `Task: ${instruction}

Terminal (step ${step}):
\`\`\`
${screen}
\`\`\`

Respond with JSON:
{
  "analysis": "your analysis",
  "plan": "your plan",
  "commands": [{"keystrokes": "...", "duration": 1.0}],
  "task_complete": false
}`;

    const response = await this.client.chat([
      { role: 'system', content: 'You are a terminal expert.' },
      { role: 'user', content: prompt }
    ]);

    return this.parseResponse(response.content);
  }

  private parseResponse(content: string): AgentResponse {
    try {
      const data = parseJsonResponse(content);
      
      return new AgentResponse({
        analysis: data.analysis || '',
        plan: data.plan || '',
        commands: (data.commands || []).map((c: any) => 
          new Command(c.keystrokes, c.duration || 1.0)
        ),
        taskComplete: data.task_complete || false
      });
    } catch (error) {
      return AgentResponse.error(`Parse error: ${error}`);
    }
  }
}

run(new LLMAgent());
```

### LLMClient Configuration

```typescript
import { LLMClient } from 'term-sdk';

// OpenRouter (default)
const client = new LLMClient({
  provider: 'openrouter',
  model: 'anthropic/claude-3-haiku',  // Optional
  apiKey: 'sk-or-...',                // Or use env var
  budget: 5.0,                        // Max $5 per session
  timeout: 300000                     // 5 minute timeout
});

// Chutes
const client = new LLMClient({
  provider: 'chutes',
  model: 'Qwen/Qwen3-32B'
});

// OpenAI
const client = new LLMClient({
  provider: 'openai',
  model: 'gpt-4o-mini'
});

// Custom endpoint
const client = new LLMClient({
  provider: 'custom',
  baseUrl: 'https://your-api.com/v1',
  apiKey: 'your-key'
});
```

### Chat Options

```typescript
const response = await client.chat(
  [
    { role: 'system', content: 'You are helpful.' },
    { role: 'user', content: 'Hello!' }
  ],
  {
    model: 'gpt-4o',        // Override model
    temperature: 0.7,       // Sampling temperature
    maxTokens: 4096         // Max response tokens
  }
);

// Response fields
console.log(response.content);           // Text response
console.log(response.promptTokens);      // Input tokens
console.log(response.completionTokens);  // Output tokens
console.log(response.cost);              // Cost in USD
console.log(response.latencyMs);         // Response time in ms
```

### Cost Tracking

```typescript
const client = new LLMClient({ budget: 10.0 });

// After making calls...
console.log(`Total cost: $${client.totalCost.toFixed(4)}`);
console.log(`Total tokens: ${client.totalTokens}`);
console.log(`Requests: ${client.requestCount}`);
```

## Advanced Patterns

### Conversation History

```typescript
class ConversationalAgent extends Agent {
  private client!: LLMClient;
  private history: Message[] = [];

  async setup(): Promise<void> {
    this.client = new LLMClient();
  }

  async step(instruction: string, screen: string, step: number): Promise<AgentResponse> {
    // Add current state to history
    this.history.push({
      role: 'user',
      content: `Step ${step}:\n${screen}`
    });

    // Keep history manageable
    if (this.history.length > 20) {
      this.history = this.history.slice(-20);
    }

    const response = await this.client.chat([
      { role: 'system', content: `Task: ${instruction}` },
      ...this.history
    ]);

    // Add response to history
    this.history.push({
      role: 'assistant',
      content: response.content
    });

    return this.parseResponse(response.content);
  }
}
```

### Error Recovery

```typescript
class RobustAgent extends Agent {
  async step(instruction: string, screen: string, step: number): Promise<AgentResponse> {
    // Detect common errors
    if (screen.includes('command not found')) {
      return new AgentResponse({
        analysis: 'Previous command not found',
        plan: 'Try alternative command',
        commands: [new Command('which python3\n', 0.3)]
      });
    }

    if (screen.includes('Permission denied')) {
      return new AgentResponse({
        analysis: 'Permission error detected',
        plan: 'Try with elevated privileges',
        commands: [new Command('sudo !!\n', 1.0)]
      });
    }

    // Normal processing...
    return await this.normalStep(instruction, screen, step);
  }
}
```

### Timeout Handling

```typescript
class TimeoutAgent extends Agent {
  private startTime = Date.now();
  private maxDurationMs = 5 * 60 * 1000; // 5 minutes

  async step(instruction: string, screen: string, step: number): Promise<AgentResponse> {
    const elapsed = Date.now() - this.startTime;
    
    if (elapsed > this.maxDurationMs) {
      return new AgentResponse({
        analysis: 'Timeout reached',
        plan: 'Force completion',
        commands: [],
        taskComplete: true
      });
    }

    // Normal processing...
  }
}
```

## Logging

Use stderr for logging (stdout is reserved for protocol):

```typescript
class MyAgent extends Agent {
  private log(message: string): void {
    console.error(`[MyAgent] ${message}`);
  }

  async step(instruction: string, screen: string, step: number): Promise<AgentResponse> {
    this.log(`Processing step ${step}`);
    
    // ...
  }
}
```

## Testing Your Agent

### Local Testing

```bash
# Test with a single task
term bench agent -a ./my_agent.ts -t /path/to/task \
    --provider openrouter \
    --model anthropic/claude-3-haiku

# For TypeScript, use tsx
npx tsx my_agent.ts  # Runs directly

# Or compile first
npx tsc my_agent.ts
node my_agent.js
```

### Unit Testing (Jest)

```typescript
import { MyAgent } from './my_agent';

describe('MyAgent', () => {
  let agent: MyAgent;

  beforeEach(async () => {
    agent = new MyAgent();
    await agent.setup();
  });

  afterEach(async () => {
    await agent.cleanup();
  });

  it('should return commands for initial step', async () => {
    const response = await agent.step('List files', '$ ', 1);
    
    expect(response.analysis).toBeTruthy();
    expect(response.commands.length).toBeGreaterThan(0);
    expect(response.taskComplete).toBe(false);
  });

  it('should detect task completion', async () => {
    const response = await agent.step(
      'Create test.txt',
      '$ cat test.txt\nHello World\n$ ',
      5
    );
    
    expect(response.taskComplete).toBe(true);
  });
});
```

## Complete Example

```typescript
#!/usr/bin/env npx tsx
/**
 * Complete LLM-powered terminal agent for Term Challenge.
 */
import { Agent, AgentResponse, Command, run, LLMClient, Message, parseJsonResponse } from 'term-sdk';

const SYSTEM_PROMPT = `You are an expert terminal agent. Complete tasks using shell commands.

Rules:
1. Analyze the terminal output carefully
2. Execute one logical step at a time
3. Verify your actions worked before proceeding
4. Use appropriate wait durations
5. Set task_complete=true only when verified complete

Respond with JSON:
{
  "analysis": "What you observe in the terminal",
  "plan": "What you will do next",
  "commands": [{"keystrokes": "command\\n", "duration": 1.0}],
  "task_complete": false
}`;

class TerminalAgent extends Agent {
  private client!: LLMClient;
  private conversation: Message[] = [];
  private model: string;

  constructor(model = 'anthropic/claude-3-haiku') {
    super();
    this.model = model;
  }

  async setup(): Promise<void> {
    this.log(`Initializing with model: ${this.model}`);
    this.client = new LLMClient({
      provider: 'openrouter',
      model: this.model,
      budget: 10.0
    });
  }

  async step(instruction: string, screen: string, step: number): Promise<AgentResponse> {
    this.log(`Step ${step}: Processing`);

    const userMsg = `Task: ${instruction}

Current Terminal (Step ${step}):
\`\`\`
${screen.slice(-2000)}
\`\`\`

What's your next action?`;

    // Update conversation
    this.conversation.push({ role: 'user', content: userMsg });

    // Keep conversation manageable
    if (this.conversation.length > 10) {
      this.conversation = this.conversation.slice(-10);
    }

    try {
      const response = await this.client.chat(
        [
          { role: 'system', content: SYSTEM_PROMPT },
          ...this.conversation
        ],
        { temperature: 0.3, maxTokens: 2048 }
      );

      this.log(`LLM response (${response.latencyMs}ms, $${response.cost.toFixed(4)})`);

      // Add to conversation
      this.conversation.push({ role: 'assistant', content: response.content });

      return this.parseResponse(response.content);
    } catch (error) {
      this.log(`LLM error: ${error}`);
      return AgentResponse.error(String(error));
    }
  }

  private parseResponse(content: string): AgentResponse {
    // Remove think blocks (Qwen models)
    content = content.replace(/<think>[\s\S]*?<\/think>/g, '').trim();

    try {
      const data = parseJsonResponse(content);

      const commands = (data.commands || []).map((cmd: any) => {
        if (typeof cmd === 'string') {
          return new Command(cmd);
        }
        return new Command(cmd.keystrokes || '', cmd.duration || 1.0);
      });

      return new AgentResponse({
        analysis: data.analysis || '',
        plan: data.plan || '',
        commands,
        taskComplete: data.task_complete || false
      });
    } catch (error) {
      this.log(`Parse error: ${error}`);
      return new AgentResponse({
        analysis: `Parse error: ${error}`,
        plan: content.slice(0, 500),
        commands: [],
        taskComplete: false
      });
    }
  }

  async cleanup(): Promise<void> {
    if (this.client) {
      this.log(
        `Session stats: $${this.client.totalCost.toFixed(4)}, ` +
        `${this.client.totalTokens} tokens, ` +
        `${this.client.requestCount} requests`
      );
    }
  }

  private log(message: string): void {
    console.error(`[TerminalAgent] ${new Date().toISOString()} ${message}`);
  }
}

// Parse command line args
const model = process.argv.includes('--model')
  ? process.argv[process.argv.indexOf('--model') + 1]
  : 'anthropic/claude-3-haiku';

run(new TerminalAgent(model));
```

## Running TypeScript Agents

```bash
# Using tsx (recommended)
npx tsx my_agent.ts

# Or with ts-node
npx ts-node my_agent.ts

# Or compile and run
npx tsc my_agent.ts
node my_agent.js
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `OPENROUTER_API_KEY` | OpenRouter API key |
| `CHUTES_API_KEY` | Chutes API key |
| `OPENAI_API_KEY` | OpenAI API key |
| `ANTHROPIC_API_KEY` | Anthropic API key |
| `LLM_API_KEY` | Generic fallback |
| `LLM_PROVIDER` | Default provider |
| `LLM_MODEL` | Default model |

## tsconfig.json

Recommended TypeScript configuration:

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "ESNext",
    "moduleResolution": "node",
    "esModuleInterop": true,
    "strict": true,
    "outDir": "./dist",
    "declaration": true
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules"]
}
```
