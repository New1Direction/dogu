#!/usr/bin/env python3
"""A tiny MCP server over stdio, for dogu's tests.

Tools:
  search_repositories  returns structured JSON, so output formatting can be checked
  wiki                 returns markdown text plus the same text inside structured JSON, like DeepWiki
  slow                 answers after 0.6 s, on its own thread, so calls made together can finish together
  count                adds 1 to a counter kept in memory, so tests can tell a warm server from a fresh one
"""
import json
import sys
import threading
import time

WRITE = threading.Lock()

TOOLS = [
    {
        "name": "search_repositories",
        "description": "Search GitHub repositories by a query.\nMore detail on a second line.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "What to look for"},
                "sort": {"type": "string", "enum": ["stars", "forks"]},
                "perPage": {"type": "number"},
            },
            "required": ["query"],
        },
        "annotations": {"readOnlyHint": True},
        "outputSchema": {
            "type": "object",
            "properties": {"query": {"type": "string"}, "items": {"type": "array", "items": {"type": "object"}}},
        },
    },
    {
        "name": "wiki",
        "description": "Like DeepWiki: readable markdown as text, and the same inside a JSON object.",
        "inputSchema": {"type": "object", "properties": {}},
        "outputSchema": {"type": "object", "properties": {"result": {"type": "string"}}},
    },
    {
        "name": "count",
        "description": "Add one to a counter and return it.",
        "inputSchema": {"type": "object", "properties": {}},
        "annotations": {"readOnlyHint": False, "destructiveHint": False},
    },
    {
        "name": "slow",
        "description": "Wait 0.6 seconds, then answer. For testing calls that run at the same time.",
        "inputSchema": {"type": "object", "properties": {}},
        "annotations": {"readOnlyHint": True},
    },
    {
        "name": "reset",
        "description": "Set the counter back to zero.",
        "inputSchema": {"type": "object", "properties": {}},
        "annotations": {"readOnlyHint": False, "destructiveHint": True},
    },
]

counter = 0


def answer(msg_id, result=None, error=None):
    out = {"jsonrpc": "2.0", "id": msg_id}
    if error is not None:
        out["error"] = error
    else:
        out["result"] = result
    with WRITE:
        sys.stdout.write(json.dumps(out) + "\n")
        sys.stdout.flush()


def call(name, args):
    global counter
    if name == "search_repositories":
        if not args.get("query"):
            return {"content": [{"type": "text", "text": "query is required"}], "isError": True}
        items = [{"name": "servers", "stars": 92000}, {"name": "python-sdk", "stars": 25000}]
        data = {"query": args.get("query"), "items": items}
        return {"content": [{"type": "text", "text": json.dumps(data)}], "structuredContent": data}
    if name == "wiki":
        text = "# Title\n- one\n- two"
        return {"content": [{"type": "text", "text": text}], "structuredContent": {"result": text}}
    if name == "slow":
        time.sleep(0.6)
        return {"content": [{"type": "text", "text": "done"}], "structuredContent": {"done": True}}
    if name == "reset":
        counter = 0
        return {"content": [{"type": "text", "text": "0"}], "structuredContent": {"count": 0}}
    if name == "count":
        counter += 1
        return {"content": [{"type": "text", "text": str(counter)}], "structuredContent": {"count": counter}}
    return {"content": [{"type": "text", "text": f"unknown tool {name}"}], "isError": True}


for line in sys.stdin:
    try:
        msg = json.loads(line)
    except ValueError:
        continue
    method, msg_id = msg.get("method"), msg.get("id")
    if msg_id is None:
        continue  # a notification
    if method == "initialize":
        answer(msg_id, {
            "protocolVersion": msg.get("params", {}).get("protocolVersion", "2025-06-18"),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "fixture", "version": "1.0.0"},
        })
    elif method == "tools/list":
        answer(msg_id, {"tools": TOOLS})
    elif method == "tools/call":
        params = msg.get("params", {})
        name, args = params.get("name"), params.get("arguments") or {}
        if name == "slow":
            # Like an async server: a slow call does not hold up the next request.
            threading.Thread(target=lambda i=msg_id: answer(i, call(name, args))).start()
        else:
            answer(msg_id, call(name, args))
    elif method == "ping":
        answer(msg_id, {})
    else:
        answer(msg_id, error={"code": -32601, "message": f"method not found: {method}"})
