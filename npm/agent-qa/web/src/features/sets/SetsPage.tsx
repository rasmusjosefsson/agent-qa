import { SetList } from './SetList'
import { SetDetail } from './SetDetail'
import { useRoute } from '@/router'

// SPA page: `/sets` = list, `/sets?id=<slug>` = detail (query-reactive, no
// document reload — the App shell swaps content client-side).
export function SetsPage() {
  const route = useRoute()
  const id = new URLSearchParams(route.search).get('id')
  return id ? <SetDetail key={id} id={id} /> : <SetList />
}

export default SetsPage
