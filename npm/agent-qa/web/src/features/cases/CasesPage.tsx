import { CaseList } from './CaseList'
import { CaseDetail } from './CaseDetail'
import { useRoute } from '@/router'

// SPA page: `/cases` = list, `/cases?id=<slug>` = detail (query-reactive, no
// document reload — the App shell swaps content client-side).
export function CasesPage() {
  const route = useRoute()
  const id = new URLSearchParams(route.search).get('id')
  return id ? <CaseDetail key={id} id={id} /> : <CaseList />
}

export default CasesPage
