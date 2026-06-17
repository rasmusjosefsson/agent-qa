#!/usr/bin/env bun
import { existsSync, readdirSync, readFileSync } from "fs";
import { resolve } from "path";

const root = resolve(process.argv[2] || "evals/results");

if (!existsSync(root)) {
  console.error(`missing results root: ${root}`);
  process.exit(1);
}

const rows: string[] = [];
for (const runId of readdirSync(root).sort()) {
  const reportPath = resolve(root, runId, "report.json");
  if (!existsSync(reportPath)) continue;
  const report = JSON.parse(readFileSync(reportPath, "utf-8"));
  rows.push([
    report.pass ? "PASS" : "FAIL",
    report.caseId || runId,
    report.model || "-",
    report.error || "",
    report.scenario || "",
  ].join("\t"));
}

console.log("status\tcase\tmodel\terror\tscenario");
for (const row of rows) console.log(row);
