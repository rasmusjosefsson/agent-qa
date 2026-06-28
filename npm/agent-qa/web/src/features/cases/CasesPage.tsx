import { CaseList } from './CaseList'
import { CaseDetail } from './CaseDetail'

// MPA page: `/cases` = list, `/cases?id=<slug>` = detail (full reload between
// the two — no client router, consistent with the other entries).
export function CasesPage() {
  const id =
    typeof window !== 'undefined'
      ? new URLSearchParams(window.location.search).get('id')
      : null
  return id ? <CaseDetail id={id} /> : <CaseList />
}

export default CasesPage
