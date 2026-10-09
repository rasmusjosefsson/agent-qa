// Prompt text builders shared by the workbench UI (web/src/features/cases/
// prompt.ts wraps these with its types) and the server (plan-run auto-repair
// seeds repair chats with buildRepairPromptText). Plain JS so the lib server
// can require it without a build step.
'use strict';

function runIntent(c) {
  return `${c.title} [${c.id}]`;
}

function buildRunPromptText(c, { apiBase, personaId, environmentId } = {}) {
  // Verbatim — imported steps may already carry their own numbering.
  const steps = (c.steps || [])
    .map((s) => s.trim())
    .filter(Boolean)
    .join('\n');

  const data = Object.entries(c.inputs || {})
    .map(([k, d]) => {
      const v = d.sensitive
        ? '<use a real test value; keep it secret / redacted in the recording>'
        : d.default != null && d.default !== ''
          ? JSON.stringify(d.default)
          : '<fill a realistic value>';
      return `  [${k}] = ${v}`;
    })
    .join('\n');

  const intent = runIntent(c);
  const linkUrl = `${apiBase}/api/cases/${encodeURIComponent(c.id)}/link`;

  const lines = [
    'Use the agent-qa skill to turn this QA test case into a replayable scenario, then link it back to the case.',
    '',
    'First load the skill: run `agent-qa skills get core`.',
    '',
    '## Target + sign-in — resolve BEFORE recording',
    '',
    'Check the chat sign-in state first:',
    '  curl -s "$AGENT_QA_BASE/api/chat/c/$AGENT_QA_CHAT_ID/connection"',
    '',
    `- Start URL: ${c.startUrl || '(none given — pick the target yourself: list environments via `curl -s "$AGENT_QA_BASE/api/environments"` and pick the one whose routes match the steps, then derive the URL from its baseUrl)'}`,
    '',
    'If `state` is "disconnected" and the target needs auth: list personas/environments via /api/personas + /api/environments, pick the matching environment, and connect BEFORE `agent-qa start` —',
    '  curl -s -X POST "$AGENT_QA_BASE/api/chat/c/$AGENT_QA_CHAT_ID/connect" -H \'content-type: application/json\' -d \'{"personaId":"<id>","environmentId":"<env>"}\'',
    'Connecting first stamps a useProfile baseline into the scenario so replays re-authenticate.',
    personaId
      ? `PERSONA PIN: connect with exactly personaId "${personaId}"${environmentId ? ` and environmentId "${environmentId}"` : ''}. Do NOT use any other persona — different personas can sign into different organizations/tenants, and this run is scoped to "${personaId}".`
      : 'Persona selection: use the persona marked `default: true`. If none is marked, use a persona already connected on this chat. NEVER pick a different persona on your own — different personas can sign into different organizations/tenants. If no default exists and the chat is anonymous, ask instead of guessing.',
    '',
    `Test case: "${c.title}"  (case id: ${c.id})`,
  ];
  if (c.preconditions) lines.push(`Preconditions: ${c.preconditions}`);
  lines.push(
    '',
    'Record these steps in a real browser, one at a time, substituting any [TOKEN] placeholders with the test data below (steps may carry their own numbering — keep it):',
    steps || '(no steps authored — derive a sensible flow from the title)'
  );
  if (data) lines.push('', 'Test data (placeholders → values):', data);
  lines.push(
    '',
    'Then verify the EXPECTED RESULT and record it as assertion step(s):',
    `Expected: ${c.expected || '(none specified — assert the outcome the steps imply)'}`,
    '',
    'Recording rules — read carefully:',
    '- `smart-click` / `smart-fill` / `smart-assert` / `smart-select` ALREADY record the step — never call `record-step` for the same action. One action = one recorded step.',
    '- Failed attempts and retries land in the buffer as real steps too. Before flushing, run `agent-qa buffer list` and `agent-qa buffer delete <id>` the dead/duplicate steps — roughly one step per test step.',
    '- `flush` appends a "page raised no uncaught exceptions" check that gates the run. If the app emits ambient uncaught errors you cannot fix (handled rejections, third-party telemetry): either flush with `--no-auto-errors` (env var AGENT_QA_NO_AUTO_ERRORS also works), or keep the check but demote it with `context.onFailure: "ignore"` so it reports without failing the run.',
    '',
    'Workflow:',
    `1. ${c.startUrl ? `Run \`agent-qa start "${intent}" --open "${c.startUrl}"\`` : `Run \`agent-qa start "${intent}"\``} to begin recording (this mints the scenario id).`,
    '2. Drive each step with a smart verb or a single `record-step`. Map every [TOKEN] placeholder to the scenario `inputs` so the data stays parameterized.',
    '3. Record assertion step(s) for the expected result.',
    '4. Prune the buffer (`buffer list`/`buffer delete`), then run `agent-qa flush` to write scenario.json.',
    '5. Report the resulting scenario id on its own line as: SCENARIO_SID=<sid>',
    '6. Link it to this case so the workbench shows the result:',
    `   curl -s -X POST "${linkUrl}" -H 'content-type: application/json' -d '{"scenarioSid":"<sid>"}'`
  );
  return lines.join('\n');
}

// Repair side of the loop: a case whose linked scenario just ran red gets a
// prompt that walks the agent through diagnose → classify → fix → re-replay.
// `run` is the failing run summary ({runId, summary, state}); `apiBase` is
// only needed to fill the personas lookup curl — server-side callers may pass
// any base since $AGENT_QA_BASE is set for chat agents anyway.
function buildRepairPromptText(c, scenario, run, apiBase, pin) {
  const sid = scenario.sid;
  const personaRule = pin && pin.personaId
    ? `Persona: use exactly "${pin.personaId}"${pin.environmentId ? ` on environment "${pin.environmentId}"` : ''} — this run is scoped to it. Do NOT use any other persona (different personas can sign into different organizations/tenants).`
    : 'Persona: reuse the persona this chat is connected as, the persona marked `default: true`, or the one the scenario\'s env.open useProfile names. NEVER pick a different persona on your own — different personas can sign into different organizations/tenants.';
  return [
    `The recorded scenario for this QA test case is failing. Use the agent-qa skill's repair loop to diagnose it, apply the right repair, and re-run until it passes — or prove a real product regression and keep it red.`,
    '',
    'First load the skill: run `agent-qa skills get core`. The repair loop is documented in references/heal.md section 5.',
    '',
    `Test case: "${c.title}"  (case id: ${c.id})`,
    `Scenario sid: ${sid}`,
    `Latest run: ${(run && run.runId) || '(unknown)'} — ${(run && (run.summary || run.state)) || 'failed'}`,
    `Expected: ${c.expected || '(none specified)'}`,
    '',
    'Loop — at most 3 repair cycles, then stop and report:',
    `1. Run \`agent-qa audit explain ${sid}\` — it prints the failed step, screenshot/snapshot paths, console + network signals, and the \`next:\` commands.`,
    '2. Classify and repair:',
    '   - locator/value drift → in-run auto-heal may already have recovered it (check heal.jsonl); otherwise `heal-respond` a corrected value + `replay --heal-from-run`, then `heal-promote --apply` when it holds.',
    `   - wrong flow/route → \`agent-qa buffer load ${sid}\`, \`buffer insert|delete|edit\` the offending steps, \`flush\`, replay. Look at sibling scenarios' proven patterns before inventing a new flow.`,
    '   - ambient uncaught-error noise only → demote that check via `buffer edit` with `context.onFailure: "ignore"` — it still reports but stops gating. Never delete assertions that verify the case.',
    '   - auth/environment failure → fix the connection and retry; do not patch the scenario around it.',
    '   - product regression → STOP, keep the run red, report the evidence.',
    `3. Replay again: \`agent-qa replay ${sid} --profile <persona>\`. ${personaRule}`,
    '4. Report one line: SCENARIO_VERDICT=<pass|blocked|regression> plus the reason.',
    '',
    `Sign-in check first: \`curl -s "${apiBase}/api/chat/c/$AGENT_QA_CHAT_ID/connection"\` — if disconnected, connect before replaying.`,
  ].join('\n');
}

module.exports = { runIntent, buildRunPromptText, buildRepairPromptText };
