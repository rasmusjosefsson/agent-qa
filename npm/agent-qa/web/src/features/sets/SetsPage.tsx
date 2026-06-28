import { SetList } from './SetList'
import { SetDetail } from './SetDetail'

// MPA page: `/sets` = list, `/sets?id=<slug>` = detail (full reload between the
// two — no client router, consistent with the other entries).
export function SetsPage() {
  const id =
    typeof window !== 'undefined'
      ? new URLSearchParams(window.location.search).get('id')
      : null
  return id ? <SetDetail id={id} /> : <SetList />
}

export default SetsPage
