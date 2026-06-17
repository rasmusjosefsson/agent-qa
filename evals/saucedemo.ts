import { parseArgs, printReport, runCase } from "./lib/harness.ts";
import { selectCases } from "./cases.ts";

async function main(): Promise<void> {
  const options = parseArgs(process.argv.slice(2));
  const [evalCase] = selectCases("saucedemo-checkout");
  const report = await runCase(evalCase, options);
  if (options.json) {
    console.log(JSON.stringify(report, null, 2));
  } else {
    printReport(report);
  }
  process.exit(report.pass ? 0 : 1);
}

main().catch((err) => {
  console.error(err instanceof Error ? err.stack || err.message : String(err));
  process.exit(1);
});
