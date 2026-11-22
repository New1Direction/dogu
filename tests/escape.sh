#!/bin/sh
# Runs tests/escape.ts in the sandbox and fails if any way out worked.
#   cargo build --release && sh tests/escape.sh
# Needs Deno and Node (for the memory server the sandbox is pointed at).
set -eu
here="$(cd "$(dirname "$0")/.." && pwd)"
M="$here/target/release/dogu"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
printf '%s\n' '{"mcpServers":{"memory":{"command":"npx","args":["-y","@modelcontextprotocol/server-memory"]}}}' > "$work/mcp.json"
"$M" sync --config "$work/mcp.json" --out "$work/mcp" > /dev/null
"$M" run --root "$work/mcp" "$here/tests/escape.ts" > "$work/out" 2>&1 || true
cat "$work/out"
expected=$(grep -c '^await attempt(' "$here/tests/escape.ts")
blocked=$(grep -c '^blocked ' "$work/out" || true)
if grep -q '^ESCAPED' "$work/out" || ! grep -q '^ts ok 42' "$work/out" || [ "$blocked" -ne "$expected" ]; then
  echo "FAIL: $blocked of $expected blocked"; exit 1
fi
echo "$blocked of $expected blocked"
