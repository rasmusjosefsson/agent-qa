// Turn agent-runtime setup failures into a sentence a person can act on.
// Scenario and tool failures stay verbatim — those are the agent's findings.
export function humanizeAgentError(message: string): string {
  const text = String(message || '').trim()
  if (/no api key found/i.test(text) || /use \/login to log in/i.test(text)) {
    return (
      'The agent has no model login, so it stopped before doing any work. ' +
      'In a terminal, run pi and then /login — or set a provider API key — ' +
      'restart the workbench, and start a new chat.'
    )
  }
  return text || 'The agent stopped.'
}
