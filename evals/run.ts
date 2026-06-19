import { cases, selectCases } from "./cases.ts";
import { parseArgs, printReport, runCase, type EvalReport, type RunSummary } from "./lib/harness.ts";

async function main(): Promise<void> {
  const options = parseArgs(process.argv.slice(2));
  const selected = selectCases(options.caseId, options.suite, options.page).slice(0, options.maxCases);

  if (selected.length === 0) {
    console.error("No eval cases matched.");
    console.error(`Available cases: ${cases.map((item) => item.id).join(", ")}`);
    process.exit(1);
  }

  if (options.list) {
    if (options.json) {
      console.log(JSON.stringify(selected, null, 2));
    } else {
      for (const evalCase of selected) {
        console.log(`${evalCase.id}\t${evalCase.suite}\t${evalCase.page || "-"}\t${evalCase.name}`);
      }
    }
    process.exit(0);
  }

  const reports: EvalReport[] = [];
  for (const evalCase of selected) {
    const report = await runCase(evalCase, options);
    reports.push(report);
    if (!options.json) printReport(report);
    if (!report.pass && !options.continueOnFailure) break;
  }

  const passed = reports.filter((report) => report.pass).length;
  const summary: RunSummary = {
    pass: passed === reports.length,
    total: reports.length,
    passed,
    failed: reports.length - passed,
    reports,
  };

  if (options.json) {
    console.log(JSON.stringify(summary, null, 2));
  } else {
    console.log(`\nSummary: ${summary.passed}/${summary.total} passed`);
  }

  process.exit(summary.pass ? 0 : 1);
}

main().catch((err) => {
  console.error(err instanceof Error ? err.stack || err.message : String(err));
  process.exit(1);
});
