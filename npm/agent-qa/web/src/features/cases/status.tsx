import { cn } from '@/lib/utils'
import type { ScenarioSummary } from '@/features/runs/types'

type Tone = 'pass' | 'fail' | 'running' | 'recorded' | 'none'

const TONE: Record<Tone, string> = {
  pass: 'border-emerald-500/30 bg-emerald-500/10 text-emerald-400',
  fail: 'border-destructive/30 bg-destructive/10 text-destructive',
  running: 'border-amber-500/30 bg-amber-500/10 text-amber-300',
  recorded: 'border-sky-500/30 bg-sky-500/10 text-sky-300',
  none: 'border-border bg-muted/40 text-muted-foreground',
}

// Derive a human status for a case from its linked scenario summary.
export function caseStatus(scenario: ScenarioSummary | null): { tone: Tone; label: string } {
  if (!scenario || !scenario.hasScenario) return { tone: 'none', label: 'Not recorded' }
  const r = scenario.latestRun
  if (r?.state === 'running') return { tone: 'running', label: 'Running' }
  if (r?.ok === true) return { tone: 'pass', label: 'Passed' }
  if (r?.ok === false) return { tone: 'fail', label: 'Failed' }
  if (!scenario.latestRunId) return { tone: 'recorded', label: 'Recorded' }
  return { tone: 'none', label: 'Unknown' }
}

export function StatusBadge({
  scenario,
  className,
}: {
  scenario: ScenarioSummary | null
  className?: string
}) {
  const { tone, label } = caseStatus(scenario)
  return (
    <span
      className={cn(
        'inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-medium',
        TONE[tone],
        className
      )}
    >
      {label}
    </span>
  )
}
