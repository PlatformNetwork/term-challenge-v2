"""
Agent runner for Term Challenge with HTTP server for persistence.
"""

import sys
import json
import traceback
import time
import signal
from http.server import HTTPServer, BaseHTTPRequestHandler
from .types import Request, Response
from .agent import Agent
from .llm import CostLimitExceeded


# Default port for HTTP server
DEFAULT_PORT = 8765

# Global agent reference for HTTP handler
_agent: Agent = None
_log_enabled = True


def set_logging(enabled: bool) -> None:
    """Enable or disable agent logging."""
    global _log_enabled
    _log_enabled = enabled


def log(msg: str) -> None:
    """Log to stderr (stdout is reserved for protocol)."""
    if _log_enabled:
        print(f"[agent] {msg}", file=sys.stderr, flush=True)


def log_error(msg: str) -> None:
    """Log an error message."""
    print(f"[agent] ERROR: {msg}", file=sys.stderr, flush=True)


class AgentHandler(BaseHTTPRequestHandler):
    """HTTP request handler for agent communication."""
    
    def log_message(self, format, *args):
        """Suppress default HTTP logging."""
        pass
    
    def do_POST(self):
        """Handle POST /step requests."""
        global _agent
        
        if self.path == '/step':
            try:
                content_length = int(self.headers.get('Content-Length', 0))
                body = self.rfile.read(content_length).decode('utf-8')
                
                # Parse request
                request = Request.parse(body)
                instruction_preview = request.instruction[:50].replace('\n', ' ')
                log(f"Step {request.step}: {instruction_preview}...")
                
                # Solve
                start_time = time.time()
                response = _agent.solve(request)
                elapsed_ms = int((time.time() - start_time) * 1000)
                
                # Log response
                if response.command:
                    cmd_preview = response.command[:60]
                    log(f"  -> {cmd_preview}{'...' if len(response.command) > 60 else ''} ({elapsed_ms}ms)")
                if response.task_complete:
                    log(f"  -> Task complete")
                
                # Send response
                response_json = response.to_json()
                self.send_response(200)
                self.send_header('Content-Type', 'application/json')
                self.send_header('Content-Length', len(response_json))
                self.end_headers()
                self.wfile.write(response_json.encode('utf-8'))
                
            except CostLimitExceeded as e:
                # Fatal error - agent must stop immediately
                log_error(f"Cost limit exceeded: {e.message}")
                log(f"Agent stopping: ${e.used:.4f} used of ${e.limit:.4f} limit")
                done_response = Response.done(
                    f"Cost limit exceeded: ${e.used:.2f} of ${e.limit:.2f}"
                ).to_json()
                self.send_response(200)
                self.send_header('Content-Type', 'application/json')
                self.send_header('Content-Length', len(done_response))
                self.end_headers()
                self.wfile.write(done_response.encode('utf-8'))
                
            except Exception as e:
                log_error(f"Error in solve(): {e}")
                traceback.print_exc(file=sys.stderr)
                # Return error command, NOT task_complete - let agent retry
                error_response = Response.cmd(
                    f"echo 'AGENT ERROR: {str(e)[:100]}'"
                ).to_json()
                self.send_response(200)  # 200 so harness processes it
                self.send_header('Content-Type', 'application/json')
                self.send_header('Content-Length', len(error_response))
                self.end_headers()
                self.wfile.write(error_response.encode('utf-8'))
        
        elif self.path == '/health':
            self.send_response(200)
            self.send_header('Content-Type', 'text/plain')
            self.end_headers()
            self.wfile.write(b'ok')
        
        elif self.path == '/shutdown':
            log("Shutdown requested")
            self.send_response(200)
            self.end_headers()
            # Signal server to stop
            raise KeyboardInterrupt()
        
        else:
            self.send_response(404)
            self.end_headers()
    
    def do_GET(self):
        """Handle GET /health for readiness check."""
        if self.path == '/health':
            self.send_response(200)
            self.send_header('Content-Type', 'text/plain')
            self.end_headers()
            self.wfile.write(b'ok')
        else:
            self.send_response(404)
            self.end_headers()


def run(agent: Agent, port: int = None) -> None:
    """
    Run an agent - auto-detects stdin mode vs HTTP server mode.
    
    If stdin is piped (not a terminal), uses stdio mode for evaluator compatibility.
    Otherwise starts HTTP server for interactive sessions.
    
    Args:
        agent: Your agent instance
        port: HTTP port (default: 8765, or AGENT_PORT env var)
    """
    import os
    
    # Check for FORCE_HTTP mode (for external agent system)
    force_http_mode = os.environ.get('FORCE_HTTP_SERVER') == '1'
    
    # Auto-detect: if stdin is piped and not forced, use stdio mode (for evaluator)
    if not sys.stdin.isatty() and not force_http_mode:
        run_stdio(agent)
        return
    
    # Otherwise use HTTP server mode
    global _agent
    
    if port is None:
        port = int(os.environ.get('AGENT_PORT', DEFAULT_PORT))
    
    _agent = agent
    
    try:
        log("Initializing agent...")
        agent.setup()
        log("Agent ready")
        
        server = HTTPServer(('0.0.0.0', port), AgentHandler)
        log(f"Listening on port {port}")
        
        def shutdown_handler(signum, frame):
            log("Received shutdown signal")
            server.shutdown()
        
        signal.signal(signal.SIGTERM, shutdown_handler)
        signal.signal(signal.SIGINT, shutdown_handler)
        
        server.serve_forever()
        
    except KeyboardInterrupt:
        log("Shutting down...")
    except Exception as e:
        log_error(f"Fatal error: {e}")
        traceback.print_exc(file=sys.stderr)
    finally:
        # Cleanup ONCE at end
        log("Cleaning up...")
        agent.cleanup()
        log("Agent finished")


# Legacy stdin/stdout mode for compatibility
def run_stdio(agent: Agent) -> None:
    """
    Run agent in stdin/stdout mode (legacy, single request).
    """
    error_count = 0
    max_errors = 5  # Don't let errors loop forever
    
    try:
        agent.setup()
        
        for line in sys.stdin:
            line = line.strip()
            if not line:
                continue
            
            try:
                request = Request.parse(line)
                response = agent.solve(request)
                print(response.to_json(), flush=True)
                error_count = 0  # Reset on success
                
                if response.task_complete:
                    break
            except CostLimitExceeded as e:
                # Fatal error - agent must stop immediately
                log_error(f"Cost limit exceeded: {e.message}")
                log(f"Agent stopping: ${e.used:.4f} used of ${e.limit:.4f} limit")
                print(Response.done(f"Cost limit exceeded: ${e.used:.2f} of ${e.limit:.2f}").to_json(), flush=True)
                break
            except Exception as e:
                error_count += 1
                log_error(f"Error in solve(): {e}")
                traceback.print_exc(file=sys.stderr)
                
                if error_count >= max_errors:
                    log_error(f"Too many errors ({max_errors}), giving up")
                    print(Response.done().to_json(), flush=True)
                    break
                
                # Return error command, not done
                print(Response.cmd(f"echo 'AGENT ERROR: {str(e)[:100]}'").to_json(), flush=True)
        
        agent.cleanup()
        
    except KeyboardInterrupt:
        pass
    except Exception as e:
        log_error(f"Fatal: {e}")
        traceback.print_exc(file=sys.stderr)
