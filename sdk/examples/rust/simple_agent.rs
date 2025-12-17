//! Simple rule-based agent example.

use term_sdk::{Agent, Request, Response, run};

struct SimpleAgent;

impl Agent for SimpleAgent {
    fn solve(&mut self, req: &Request) -> Response {
        // First step: explore
        if req.is_first() {
            return Response::cmd("ls -la");
        }

        // Check for errors
        if req.failed() {
            return Response::cmd("pwd");
        }

        // Example: create hello.txt task
        if req.instruction.to_lowercase().contains("hello") {
            if req.step == 2 {
                return Response::cmd("echo 'Hello, world!' > hello.txt");
            }
            if req.step == 3 {
                return Response::cmd("cat hello.txt");
            }
            if req.has("Hello") {
                return Response::done();
            }
        }

        // Default: complete after exploration
        if req.step > 5 {
            return Response::done();
        }

        Response::cmd("pwd")
    }
}

fn main() {
    run(&mut SimpleAgent);
}
