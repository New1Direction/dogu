# Forge benchmark 2026-11-22

dogu built at 5c081de (it reported 0.3.0; forge included), `sonnet` forges in session 1, `haiku` does every other session, 3 worlds per setup and condition, bench at commit 4092181. Job: the latest release of each of an org's 3 most-starred repos; orgs in order: tokio-rs, denoland, vercel, rust-lang, modelcontextprotocol.

| Setup | Condition | Right | Cost per job, sessions 3-5 (mean) | Time, sessions 3-5 (median) | Turns, sessions 3-5 (median) | Sessions 1-2 cost per world | Used the forged tool (sessions 2-5) |
|---|---|---|---|---|---|---|---|
| folder | forge | 15/15 | $0.0008 | 6.1s | 2 | $0.0196 | 12/12 |
| folder | no-forge | 15/15 | $0.0012 | 7.5s | 3 | $0.0028 | - |
| codemode | forge | 15/15 | $0.0010 | 7.0s | 4 | $0.0249 | 12/12 |
| codemode | no-forge | 15/15 | $0.0012 | 8.5s | 3 | $0.0024 | - |

- folder: forging costs $0.0168 once (the forging session and the next session's cache refresh) and then saves $0.0004 per job (35%), so it pays for itself after 42 more job(s).
- codemode: forging costs $0.0225 once (the forging session and the next session's cache refresh) and then saves $0.0002 per job (18%), so it pays for itself after 102 more job(s).
