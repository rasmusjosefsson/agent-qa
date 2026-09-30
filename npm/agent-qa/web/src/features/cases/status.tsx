import { cn } from '@/lib/utils'
import type { ScenarioSummary } from '@/features/runs/types'

type Tone = 'pass' | 'fail' | 'running' | 'recorded' | 'none'

const TONE: Record<Tone, string> = {
  pass: 'border-success/25 bg-success/10 text-success',
  fail: 'border-destructive/25 bg-destructive/10 text-destructive',
  running: 'border-warning/30 bg-warning/10 text-warning',
  recorded: 'border-info/25 bg-info/10 text-info',
  none: 'border-border bg-muted/60 text-muted-foreground',
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
        'inline-flex items-center gap-1.5 whitespace-nowrap rounded-full border px-2 py-0.5 text-xs font-medium',
        TONE[tone],
        className
      )}
    >
      <span className={cn('size-1.5 rounded-full bg-current', tone === 'running' && 'aqa-ping')} />
      {label}
    </span>
  )
}
