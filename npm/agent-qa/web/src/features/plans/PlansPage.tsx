import { PlanList } from './PlanList'
import { PlanDetail } from './PlanDetail'
import { useRoute } from '@/router'

// SPA page: `/plans` = list, `/plans?id=<slug>` = detail (query-reactive, no
// document reload — the App shell swaps content client-side).
export function PlansPage() {
  const route = useRoute()
  const id = new URLSearchParams(route.search).get('id')
  return id ? <PlanDetail key={id} id={id} /> : <PlanList />
}

export default PlansPage
