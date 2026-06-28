// web/src/features/cases/tokens.ts
// [TOKEN] test-data placeholders. A token is an UPPER_SNAKE name in brackets,
// e.g. [EMAIL], [PASSWORD], [FILE_DEMO_PNG]. They map 1:1 to scenario `inputs`.
import type { InputDecl } from './types'

export const TOKEN_RE = /\[([A-Z][A-Z0-9_]*)\]/g

export function extractTokens(...texts: (string | string[])[]): string[] {
  const seen = new Set<string>()
  const push = (t: string) => {
    for (const m of t.matchAll(TOKEN_RE)) seen.add(m[1])
  }
  for (const t of texts) {
    if (Array.isArray(t)) t.forEach(push)
    else push(t)
  }
  return [...seen]
}

// Sensible default: treat secret-ish tokens as sensitive (blank default).
export function isSensitiveName(name: string): boolean {
  return /(PASS|PWD|SECRET|TOKEN|KEY|OTP|PIN)/.test(name)
}

// Reconcile the inputs map with the tokens currently present in the text:
// add newly-typed tokens (with a guessed sensitivity), drop tokens no longer
// referenced, and preserve the author's edits for the ones that remain.
export function reconcileInputs(
  tokens: string[],
  prev: Record<string, InputDecl>
): Record<string, InputDecl> {
  const next: Record<string, InputDecl> = {}
  for (const t of tokens) {
    next[t] = prev[t] ?? { type: 'string', default: '', sensitive: isSensitiveName(t) }
  }
  return next
}

// Split a token-annotated string into plain/token segments for highlighting.
export function tokenSegments(text: string): { text: string; token: boolean }[] {
  const out: { text: string; token: boolean }[] = []
  let last = 0
  for (const m of text.matchAll(TOKEN_RE)) {
    const i = m.index ?? 0
    if (i > last) out.push({ text: text.slice(last, i), token: false })
    out.push({ text: m[0], token: true })
    last = i + m[0].length
  }
  if (last < text.length) out.push({ text: text.slice(last), token: false })
  return out
}
