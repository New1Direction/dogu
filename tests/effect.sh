#!/bin/sh
# dogu against an MCP server built with Effect 4 (tests/fixtures/effect_mcp_server.ts): over stdio and
# Streamable HTTP, in both protocol eras (2025-11-25 and the stateless 2026-07-28).
#   cargo build --release && sh tests/effect.sh
# Needs Deno (it fetches effect@4.1.5 and @effect/platform-deno@4.1.5 from npm the first time).
set -eu
here="$(cd "$(dirname "$0")/.." && pwd)"
M="${DOGU_BIN:-$here/target/release/dogu}"
SERVER="$here/tests/fixtures/effect_mcp_server.ts"
work="$(mktemp -d)"
pids=""
trap 'for p in $pids; do kill "$p" 2>/dev/null || true; done; rm -rf "$work"' EXIT
export DOGU_WARM=0 # each command starts its own connection, so nothing outlives the test

fail() { echo "FAIL ($1): $2"; exit 1; }

# The program: calls started together, one that fails among them, and a repeat of a read-only call,
# which dogu answers from its cache. It prints how many times the server really ran `add`.
PROGRAM='
const before = await tools.eff.calls({});
const [x, y, p] = await Promise.all([tools.eff.add({ a: 1, b: 1 }), tools.eff.add({ a: 2, b: 2 }), tools.eff.lookup({ name: "ada" })]);
const again = await tools.eff.add({ a: 1, b: 1 });
const rs = await Promise.allSettled([tools.eff.lookup({ name: "grace" }), tools.eff.lookup({ name: "nobody" })]);
const born = rs.map((r) => (r.status === "fulfilled" ? r.value.born : "none")).join(",");
console.log(x, y, p.born, again, born, "ran add", (await tools.eff.calls({})) - before);'
EXPECTED='2 4 1815 2 1906,none ran add 2'

check() { # $1 name, $2 folder
  name=$1; root=$2
  for t in add lookup calls; do [ -x "$root/eff/$t" ] || fail "$name" "sync made no eff/$t"; done
  grep -q 'eff/add --a <number> --b <number>' "$root/README.md" || fail "$name" "README does not show number flags"
  grep 'eff/lookup' "$root/README.md" | grep -q 'read-only' || fail "$name" "lookup is not marked read-only"
  [ "$("$root/eff/add" --a 2 --b 3)" = "5" ] || fail "$name" "eff/add --a 2 --b 3 is not 5"
  if out=$("$root/eff/lookup" --name nobody 2>&1); then fail "$name" "a failed call exited 0"; fi
  echo "$out" | grep -q 'no person named nobody' || fail "$name" "the server's error was lost: $out"
  got=$("$root/run" -e "$PROGRAM" 2>&1) || fail "$name" "the program failed: $got"
  [ "$got" = "$EXPECTED" ] || fail "$name" "the program printed '$got', expected '$EXPECTED'"
  echo "pass  $name"
}

port=18760
for era in 2025-11-25 2026-07-28; do
  dir="$work/stdio-$era"; mkdir -p "$dir"
  printf '{"mcpServers":{"eff":{"command":"deno","args":["run","-A","%s","stdio","%s"]}}}' "$SERVER" "$era" > "$dir/mcp.json"
  "$M" sync --config "$dir/mcp.json" --out "$dir/mcp" > /dev/null || fail "stdio $era" "sync failed"
  check "stdio $era" "$dir/mcp"

  port=$((port + 1))
  dir="$work/http-$era"; mkdir -p "$dir"
  deno run -A "$SERVER" http "$era" "$port" > "$dir/server.log" 2>&1 &
  pids="$pids $!"
  up=no
  for _ in $(seq 1 120); do
    if curl -s -o /dev/null "http://127.0.0.1:$port/mcp"; then up=yes; break; fi
    sleep 0.5
  done
  [ "$up" = yes ] || fail "http $era" "the server did not start: $(cat "$dir/server.log")"
  printf '{"mcpServers":{"eff":{"url":"http://127.0.0.1:%s/mcp"}}}' "$port" > "$dir/mcp.json"
  "$M" sync --config "$dir/mcp.json" --out "$dir/mcp" > /dev/null || fail "http $era" "sync failed"
  check "http $era" "$dir/mcp"
done
echo "4 of 4 passed"
