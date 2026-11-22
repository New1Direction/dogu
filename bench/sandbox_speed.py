#!/usr/bin/env python3
"""Time the sandbox's two engines, djs and Deno, on the same dogu binary.

    python3 bench/sandbox_speed.py target/release/dogu <a synced folder with the test server as "fx">

Each program runs 20 times per engine; the median is printed. The test server is
tests/fixtures/mcp_server.py.
"""
import subprocess, time, os, statistics, sys
B, root = sys.argv[1], sys.argv[2]
progs = {
  "trivial": "console.log(1)",
  "10 tool calls": "let n = 0; for (let i = 0; i < 10; i++) { n = (await tools.fx.count({})).count; } console.log(n)",
  "compute": "let s = 0; for (let i = 0; i < 2000000; i++) s += i % 7; console.log(s)",
  "json 20k items": "const items = Array.from({ length: 20000 }, (_, i) => ({ id: i, name: 'repo-' + i, stars: (i * 7919) % 100000, tags: ['a', String(i % 13)] })); const back = JSON.parse(JSON.stringify(items)); console.log(back.filter((r) => r.tags.includes('7')).sort((a, b) => b.stars - a.stars).slice(0, 5).map((r) => r.name).join(','))",
}
for name, code in progs.items():
    row = []
    for eng in ["deno", "djs"]:
        env = dict(os.environ, DOGU_SANDBOX=eng)
        times = []
        for _ in range(20):
            t = time.perf_counter()
            r = subprocess.run([B, "run", "--root", root, "-e", code], env=env, capture_output=True, text=True)
            times.append(time.perf_counter() - t)
            assert r.returncode == 0, (eng, name, r.stderr[:200])
        row.append((eng, statistics.median(times) * 1000, r.stdout.strip()))
    d, j = row[0][1], row[1][1]
    print(f"{name:14} deno {d:7.1f} ms   djs {j:7.1f} ms   ({d / j:4.1f}x)   output deno={row[0][2]!r} djs={row[1][2]!r}")
