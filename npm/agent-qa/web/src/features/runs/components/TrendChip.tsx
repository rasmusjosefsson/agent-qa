// web/src/features/runs/components/TrendChip.tsx
// `audit trend` in the header: the scenario's last N runs as a ✓/✗ glyph
// line + a duration sparkline, so "is this degrading" is visible without
// leaving the Runs pane. Hidden when the scenario has no replayed runs.
import { useEffect, useState } from 'react'
import { getTrend } from '@/lib/runs-api'
import type { AuditTrend } from '../types'

export function TrendChip({ sid }: { sid: string }) {
  const [trend, setTrend] = useState<AuditTrend | null>(null)

  useEffect(() => {
    let live = true
    setTrend(null)
    getTrend(sid)
      .then((r) => {
        if (live && r.trend && r.trend.outcomes) setTrend(r.trend)
      })
      .catch(() => {})
    return () => {
      live = false
    }
  }, [sid])

  if (!trend) return null
  const total = trend.passed + trend.failed
  const pct = total > 0 ? Math.round((trend.passed / total) * 100) : 0
  const glyphs = [...trend.outcomes]
  return (
    <span
      className="inline-flex items-center gap-1 font-mono text-[10px] text-muted-foreground"
      title={`audit trend — pass ${pct}% (${trend.passed}/${total}), median ${trend.medianSecs.toFixed(2)}s over the last ${trend.runs.length} run(s)`}
    >
      <span aria-hidden>·</span>
      <span aria-label="recent outcomes">
        {glyphs.map((g, i) => (
          <span key={i} className={g === '✓' ? 'text-emerald-500' : 'text-rose-500'}>
            {g}
          </span>
        ))}
      </span>
      <span aria-label="duration trend">{trend.sparkline}</span>
    </span>
  )
}
