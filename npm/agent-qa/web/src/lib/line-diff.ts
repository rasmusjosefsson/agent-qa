// Line diff for ARIA snapshots (domshot baselines vs a run).
// Same algorithm as the standalone viewer in site/public/diff-viewer.html —
// keep the two in step when the pairing rules change.

export interface LineSpan {
  text: string
  changed: boolean
}

export interface DiffRow {
  kind: 'same' | 'add' | 'del' | 'change'
  before?: string
  after?: string
  beforeSpans?: LineSpan[]
  afterSpans?: LineSpan[]
}

interface Op {
  t: 'eq' | 'del' | 'add'
  text: string
}

/** Drop one trailing newline so a shared terminator is not a phantom blank line. */
export function splitLines(text: string): string[] {
  const stripped = text.endsWith('\n') ? text.slice(0, -1) : text
  if (stripped === '') return []
  return stripped.split('\n')
}

// Full LCS is O(n*m). Past this cell count, pair lines by index instead of
// allocating a giant table — a snapshot that large is already unreadable
// as an aligned diff, and the fallback still marks equal lines.
const MAX_CELLS = 800_000
const MAX_TOKEN_CELLS = 20_000

export function lineDiff(before: string, after: string): DiffRow[] {
  const a = splitLines(before)
  const b = splitLines(after)
  const ops = a.length * b.length > MAX_CELLS ? zipOps(a, b) : lcsOps(a, b)
  return pairOps(ops)
}

function lcsOps(a: string[], b: string[]): Op[] {
  const n = a.length
  const m = b.length
  const dp: number[][] = Array.from({ length: n + 1 }, () => new Array<number>(m + 1).fill(0))
  for (let i = n - 1; i >= 0; i--) {
    const row = dp[i]
    const next = dp[i + 1]
    for (let j = m - 1; j >= 0; j--) {
      row[j] = a[i] === b[j] ? next[j + 1] + 1 : Math.max(next[j], row[j + 1])
    }
  }
  const ops: Op[] = []
  let i = 0
  let j = 0
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      ops.push({ t: 'eq', text: a[i] })
      i++
      j++
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      ops.push({ t: 'del', text: a[i] })
      i++
    } else {
      ops.push({ t: 'add', text: b[j] })
      j++
    }
  }
  while (i < n) ops.push({ t: 'del', text: a[i++] })
  while (j < m) ops.push({ t: 'add', text: b[j++] })
  return ops
}

function zipOps(a: string[], b: string[]): Op[] {
  const ops: Op[] = []
  const n = Math.max(a.length, b.length)
  for (let i = 0; i < n; i++) {
    if (i < a.length && i < b.length) {
      if (a[i] === b[i]) ops.push({ t: 'eq', text: a[i] })
      else {
        ops.push({ t: 'del', text: a[i] })
        ops.push({ t: 'add', text: b[i] })
      }
    } else if (i < a.length) ops.push({ t: 'del', text: a[i] })
    else ops.push({ t: 'add', text: b[i] })
  }
  return ops
}

function pairOps(ops: Op[]): DiffRow[] {
  const rows: DiffRow[] = []
  let i = 0
  while (i < ops.length) {
    if (ops[i].t === 'eq') {
      rows.push({ kind: 'same', before: ops[i].text, after: ops[i].text })
      i++
      continue
    }
    const dels: string[] = []
    const adds: string[] = []
    while (i < ops.length && ops[i].t !== 'eq') {
      if (ops[i].t === 'del') dels.push(ops[i].text)
      else adds.push(ops[i].text)
      i++
    }
    const n = Math.max(dels.length, adds.length)
    for (let k = 0; k < n; k++) {
      const d = dels[k]
      const a = adds[k]
      if (d !== undefined && a !== undefined) {
        const spans = tokenSpans(d, a)
        rows.push({
          kind: 'change',
          before: d,
          after: a,
          beforeSpans: spans.before,
          afterSpans: spans.after,
        })
      } else if (d !== undefined) rows.push({ kind: 'del', before: d })
      else rows.push({ kind: 'add', after: a })
    }
  }
  return rows
}

function tokenize(s: string): string[] {
  return s.split(/(\s+)/).filter((t) => t.length > 0)
}

function tokenSpans(a: string, b: string): { before: LineSpan[]; after: LineSpan[] } {
  const ta = tokenize(a)
  const tb = tokenize(b)
  if (ta.length * tb.length > MAX_TOKEN_CELLS) {
    return {
      before: [{ text: a, changed: a !== b }],
      after: [{ text: b, changed: a !== b }],
    }
  }
  const ops = lcsOps(ta, tb)
  const before: LineSpan[] = []
  const after: LineSpan[] = []
  const push = (spans: LineSpan[], text: string, changed: boolean) => {
    const prev = spans[spans.length - 1]
    if (prev && prev.changed === changed) prev.text += text
    else spans.push({ text, changed })
  }
  // Pair adjacent del/add tokens so a renamed word highlights on both sides.
  let i = 0
  while (i < ops.length) {
    if (ops[i].t === 'eq') {
      push(before, ops[i].text, false)
      push(after, ops[i].text, false)
      i++
      continue
    }
    while (i < ops.length && ops[i].t === 'del') {
      push(before, ops[i].text, true)
      i++
    }
    while (i < ops.length && ops[i].t === 'add') {
      push(after, ops[i].text, true)
      i++
    }
  }
  return { before, after }
}
