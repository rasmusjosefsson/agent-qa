// Copy for the read-only demo. The banner is the sentence readers see
// first. The tour walks the Runs view in a fixed order.

export const DEMO_BANNER = 'Demo, run agent-qa locally to record your own'

export const DEMO_QUICKSTART = '/agent-qa/docs/quickstart/'

export interface TourStep {
  id: string
  title: string
  body: string
}

export const TOUR_STEPS: TourStep[] = [
  {
    id: 'runs',
    title: 'Runs list',
    body: 'Recorded scenarios are listed here, with the latest verdict. Open one to see its replays.',
  },
  {
    id: 'run',
    title: 'Run detail',
    body: 'A run is one execution. The summary counts the checks, and each row is a step from that replay.',
  },
  {
    id: 'playback',
    title: 'Timeline playback',
    body: 'Play walks events.jsonl on the recorded timing. Pause, drag the timeline, or change the speed.',
  },
  {
    id: 'evidence',
    title: 'Screenshots and snapshots',
    body: 'Each step keeps a screenshot and an ARIA snapshot from that moment. Open a step to read them.',
  },
  {
    id: 'checks',
    title: 'Checks and claims',
    body: 'Check steps are the assertions. A failed claim names the step and the reason it missed.',
  },
  {
    id: 'heals',
    title: 'Heals',
    body: 'When a locator drifts, a heal records the correction on that step. A passing run can still have been healed.',
  },
  {
    id: 'diffs',
    title: 'Diffs',
    body: 'A missed screenshot shows the pixel diff against the baseline. Compare two runs after you record locally.',
  },
  {
    id: 'local',
    title: 'Run it locally',
    body: 'This demo only replays the bundled traces. Install agent-qa and run agent-qa web to record your own.',
  },
]
