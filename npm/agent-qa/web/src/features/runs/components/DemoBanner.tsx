import { withBase } from '@/router'
import { navigate } from '@/router'
import { DEMO_BANNER, DEMO_QUICKSTART } from '../demo-tour'

export const DEMO_TOUR_EVENT = 'aqa-start-tour'
export const DEMO_TOUR_KEY = 'aqa-demo-tour'

export function startDemoTour() {
  sessionStorage.setItem(DEMO_TOUR_KEY, '1')
  navigate(withBase('/'))
  window.dispatchEvent(new Event(DEMO_TOUR_EVENT))
}

export function DemoBanner() {
  return (
    <div
      data-tour="local"
      className="flex shrink-0 flex-wrap items-center gap-x-3 gap-y-1 border-b border-primary/30 bg-primary/10 px-3 py-2 text-[13px] text-foreground"
    >
      <p className="min-w-0 flex-1">{DEMO_BANNER}</p>
      <a href={DEMO_QUICKSTART} className="font-medium text-primary underline-offset-2 hover:underline">
        Quickstart
      </a>
      <button
        type="button"
        onClick={startDemoTour}
        className="rounded-md bg-primary px-2.5 py-1 text-xs font-medium text-primary-foreground hover:bg-primary/90"
      >
        Take the tour
      </button>
    </div>
  )
}
