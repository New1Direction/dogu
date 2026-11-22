#!/usr/bin/env python3
"""A tiny MCP server over HTTP, for dogu's tests. It prints its port, then serves.

Every answer comes as an event stream sent in chunks, as GitHub's server sends them. Tools:
  ping          answers "pong"
  connections   how many separate TCP connections have reached the server so far
  hold          answers at once, then keeps its stream open for 3 seconds
"""
import json
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

SEEN = set()
TOOLS = [
    {"name": "ping", "description": "Answer pong.", "inputSchema": {"type": "object", "properties": {}}},
    {"name": "connections", "description": "Count connections.", "inputSchema": {"type": "object", "properties": {}}},
    {"name": "hold", "description": "Answer, then hold the stream open.", "inputSchema": {"type": "object", "properties": {}}},
]


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"  # keep connections open between requests

    def log_message(self, *args):
        pass

    def do_POST(self):
        SEEN.add(self.client_address)
        msg = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))))
        if "id" not in msg:
            self.send_response(202)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        method = msg.get("method")
        if method == "initialize":
            result = {"protocolVersion": msg["params"]["protocolVersion"], "capabilities": {"tools": {}},
                      "serverInfo": {"name": "web", "version": "1"}}
        elif method == "tools/list":
            result = {"tools": TOOLS}
        elif method == "tools/call":
            name = msg["params"]["name"]
            text = str(len(SEEN)) if name == "connections" else "pong"
            result = {"content": [{"type": "text", "text": text}]}
        else:
            self.answer({"jsonrpc": "2.0", "id": msg["id"], "error": {"code": -32601, "message": "unknown method"}})
            return
        hold = method == "tools/call" and msg["params"]["name"] == "hold"
        self.answer({"jsonrpc": "2.0", "id": msg["id"], "result": result}, hold)

    def answer(self, body, hold=False):
        data = f"event: message\ndata: {json.dumps(body)}\n\n".encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Transfer-Encoding", "chunked")
        self.end_headers()
        self.wfile.write(f"{len(data):x}\r\n".encode() + data + b"\r\n")
        self.wfile.flush()
        if hold:
            time.sleep(3)
        self.wfile.write(b"0\r\n\r\n")  # the end of the stream, in its own write


server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
print(server.server_address[1], flush=True)
server.serve_forever()
