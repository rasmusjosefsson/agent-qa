// A {"shot":"<id>"} check fails on a different step than the screenshot it
// compared. The error names that id (`shot 's1' differs`). The image and the
// diff have to use the same id, not the check step's own sidecar and not
// whichever frame the timeline happened to open on.

export function shotIdFromError(error: string | undefined): string | undefined {
  const match = error?.match(/\bshot '([^']+)'/)
  return match?.[1]
}

export function evidenceStepId(
  step: { id?: string; error?: string },
  claimShot?: string | null,
): string | undefined {
  if (claimShot) return claimShot
  return shotIdFromError(step.error) ?? step.id
}

export function isShotMiss(
  step: { status?: string; error?: string },
  claimShot?: string | null,
): boolean {
  if (step.status !== 'fail') return false
  if (claimShot) return true
  return shotIdFromError(step.error) != null
}

/** A viewport step is captured before any page exists, so its file is blank. */
export function hasPageFrame(step: { kind?: string; screenshot?: string }): boolean {
  if (!step.screenshot) return false
  return !/(^|:)viewport$/.test(step.kind || '')
}

/**
 * A failed run opens on the failure. A passing run opens on the first step
 * that actually shows a page, skipping a blank viewport frame.
 */
export function focusStep<T extends { status?: string; kind?: string; screenshot?: string }>(
  steps: T[],
): T | undefined {
  const failed = steps.find((step) => step.status === 'fail')
  if (failed) return failed
  return steps.find((step) => hasPageFrame(step)) ?? steps[0]
}
