// Declared scenario inputs, as scenario.json stores them.
export interface InputDecl {
  type?: string
  default?: unknown
  description?: string
  sensitive?: boolean
}

// Inputs a person can edit on Replay. Secrets stay out of the form.
export function visibleInputs(
  declared: Record<string, InputDecl> | undefined,
): { name: string; value: string; description?: string }[] {
  if (!declared) return []
  return Object.entries(declared)
    .filter(([, decl]) => !decl?.sensitive)
    .map(([name, decl]) => ({
      name,
      value: decl?.default == null ? '' : String(decl.default),
      description: decl?.description,
    }))
}

// Params for one replay. Drafts (what the form shows) win over the
// environment. Keys the scenario does not declare are left out, so a
// nightly environment can carry a flag that only some members use.
export function replayParams(
  declared: Record<string, InputDecl> | undefined,
  envParams: Record<string, string> | undefined,
  drafts: Record<string, string> | undefined,
): Record<string, string> | undefined {
  if (!declared) return undefined
  const out: Record<string, string> = {}
  for (const name of Object.keys(declared)) {
    if (drafts && Object.prototype.hasOwnProperty.call(drafts, name)) out[name] = drafts[name]
    else if (envParams && Object.prototype.hasOwnProperty.call(envParams, name)) out[name] = envParams[name]
  }
  return Object.keys(out).length ? out : undefined
}
