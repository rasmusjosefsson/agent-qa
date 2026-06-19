#!/usr/bin/env bun
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const resultsRoot = resolve(evalsRoot, "results");

interface CaseSummary {
  tc: string;
  pass: boolean;
  exitCode: number;
  durationMs: number;
  attempt: number;
  resultRoot?: string;
  scenario?: string;
  replay?: string;
  error?: string;
  stdoutPath: string;
  stderrPath: string;
}

const cases = Array.from({ length: 26 }, (_, index) => `tc${String(index + 1).padStart(2, "0")}`);
const maxAttempts = Math.max(1, Number(process.env.AGENT_QA_GOLDEN_CASE_ATTEMPTS || "3"));

function stamp(): string {
  return new Date().toISOString().replace(/[:.]/g, "-");
}

function latestReport(tc: string): { path: string; report: any } | undefined {
  const prefix = `golden-automation-exercise-${tc}-`;
  const dirs = readdirSync(resultsRoot)
    .filter((dir) => dir.startsWith(prefix))
    .sort()
    .reverse();
  for (const dir of dirs) {
    const path = resolve(resultsRoot, dir, "golden-report.json");
    if (!existsSync(path)) continue;
    return { path, report: JSON.parse(readFileSync(path, "utf-8")) };
  }
  return undefined;
}

function replayPath(report: any): string | undefined {
  const sid = report?.sid;
  const scenariosRoot = report?.scenariosRoot;
  if (!sid || !scenariosRoot) return undefined;
  const replaysRoot = resolve(scenariosRoot, sid, "replays");
  if (!existsSync(replaysRoot)) return undefined;
  const latest = resolve(replaysRoot, "latest.txt");
  if (!existsSync(latest)) return undefined;
  const runId = readFileSync(latest, "utf-8").trim();
  if (!runId) return undefined;
  return resolve(replaysRoot, runId, "audit.json");
}

async function runCaseAttempt(tc: string, suiteRoot: string, attempt: number): Promise<CaseSummary> {
  const started = performance.now();
  const stdoutPath = resolve(suiteRoot, `${tc}.attempt-${attempt}.stdout.txt`);
  const stderrPath = resolve(suiteRoot, `${tc}.attempt-${attempt}.stderr.txt`);
  const proc = Bun.spawn(["bun", "run", `golden/automation-exercise-${tc}.ts`], {
    cwd: evalsRoot,
    stdout: "pipe",
    stderr: "pipe",
    env: process.env,
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  writeFileSync(stdoutPath, stdout, "utf-8");
  writeFileSync(stderrPath, stderr, "utf-8");

  const found = latestReport(tc);
  const report = found?.report;
  const scenario = report?.sid && report?.scenariosRoot
    ? resolve(report.scenariosRoot, report.sid, "scenario.json")
    : undefined;
  return {
    tc,
    pass: exitCode === 0 && Boolean(report?.pass),
    exitCode,
    durationMs: Math.round(performance.now() - started),
    attempt,
    resultRoot: report?.resultRoot,
    scenario,
    replay: report ? replayPath(report) : undefined,
    error: report?.error || (exitCode === 0 ? undefined : `process exited ${exitCode}`),
    stdoutPath,
    stderrPath,
  };
}

async function runCase(tc: string, suiteRoot: string): Promise<CaseSummary> {
  let last: CaseSummary | undefined;
  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    if (attempt > 1) console.error(`[golden-suite] RETRY ${tc} attempt=${attempt}/${maxAttempts}`);
    last = await runCaseAttempt(tc, suiteRoot, attempt);
    if (last.pass) return last;
  }
  return last!;
}

async function main(): Promise<void> {
  const suiteRoot = resolve(resultsRoot, `golden-automation-exercise-suite-${stamp()}`);
  mkdirSync(suiteRoot, { recursive: true });

  const summaries: CaseSummary[] = [];
  for (const tc of cases) {
    console.error(`[golden-suite] START ${tc}`);
    const summary = await runCase(tc, suiteRoot);
    summaries.push(summary);
    console.error(`[golden-suite] END ${tc} ${summary.pass ? "PASS" : "FAIL"} duration=${summary.durationMs}ms`);
    if (!summary.pass) {
      console.error(`[golden-suite] first failure ${tc}: ${summary.error || "unknown error"}`);
      break;
    }
  }

  const passed = summaries.filter((item) => item.pass).length;
  const summary = {
    pass: passed === cases.length,
    total: cases.length,
    completed: summaries.length,
    passed,
    failed: summaries.length - passed,
    maxAttempts,
    suiteRoot,
    cases: summaries,
  };
  writeFileSync(resolve(suiteRoot, "summary.json"), JSON.stringify(summary, null, 2), "utf-8");
  console.log(JSON.stringify(summary, null, 2));
  process.exit(summary.pass ? 0 : 1);
}

main().catch((err) => {
  console.error(err instanceof Error ? err.stack || err.message : String(err));
  process.exit(1);
});
