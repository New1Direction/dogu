// Re-check saved answers after a check was fixed, without running any agent again.
//
//   deno run --allow-read --allow-write bench/rescore.ts bench/results/<name>.json "<why>"
//
// Every run keeps the agent's full answer and the facts it was checked against, so a fixed check can
// be applied to the same answers. The results file says that it was re-scored, and why.

import { TASKS } from "./tasks.ts";

const [file, why] = Deno.args;
if (!file || !why) throw new Error("usage: rescore.ts <results.json> <why>");
const json = JSON.parse(Deno.readTextFileSync(file));
let changed = 0;
const records = json.records.map((r: { task: string; correct: boolean; metrics?: { answer: string } }) => {
  const task = TASKS.find((t) => t.id === r.task);
  if (!task || !r.metrics) return r;
  const correct = task.check(r.metrics.answer, json.answers[r.task]);
  if (correct !== r.correct) changed++;
  return { ...r, correct };
});
const rescored = [...(json.rescored ?? []), { date: new Date().toISOString().slice(0, 10), why, changed }];
Deno.writeTextFileSync(file, JSON.stringify({ ...json, records, rescored }, null, 2));
console.log(`${file}: ${changed} run(s) changed`);
