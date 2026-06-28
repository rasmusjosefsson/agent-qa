import { PlanList } from './PlanList'
import { PlanDetail } from './PlanDetail'

// MPA page: `/plans` = list, `/plans?id=<slug>` = detail (full reload between
// the two — no client router, consistent with the other entries).
export function PlansPage() {
  const id =
    typeof window !== 'undefined'
      ? new URLSearchParams(window.location.search).get('id')
      : null
  return id ? <PlanDetail id={id} /> : <PlanList />
}

export default PlansPage
