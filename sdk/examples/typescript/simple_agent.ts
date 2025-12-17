#!/usr/bin/env npx ts-node
/**
 * Simple rule-based agent example.
 */
import { Agent, Request, Response, run } from '../../typescript/src/index.js';

class SimpleAgent implements Agent {
  solve(req: Request): Response {
    // First step: explore
    if (req.first) {
      return Response.cmd("ls -la");
    }

    // Check for errors
    if (req.failed) {
      return Response.cmd("pwd");
    }

    // Example: create hello.txt task
    if (req.instruction.toLowerCase().includes("hello")) {
      if (req.step === 2) {
        return Response.cmd("echo 'Hello, world!' > hello.txt");
      }
      if (req.step === 3) {
        return Response.cmd("cat hello.txt");
      }
      if (req.has("Hello")) {
        return Response.done();
      }
    }

    // Default: complete after exploration
    if (req.step > 5) {
      return Response.done();
    }

    return Response.cmd("pwd");
  }
}

run(new SimpleAgent());
