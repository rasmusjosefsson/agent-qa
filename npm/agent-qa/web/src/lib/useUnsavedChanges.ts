import { useEffect, useRef } from 'react'
import { setNavGuard } from '@/router'

// Warn before an in-app navigation or a tab close while `dirty` is true.
// Returns `markClean`, which the caller must invoke after a successful save
// that navigates in the same turn — React has not re-rendered yet, so the
// guard would still see the pre-save dirty flag.
export function useUnsavedChanges(dirty: boolean, message: string): () => void {
  const dirtyRef = useRef(dirty)
  dirtyRef.current = dirty
  const markClean = () => {
    dirtyRef.current = false
  }

  useEffect(() => {
    setNavGuard(() => {
      if (!dirtyRef.current) return true
      return window.confirm(message)
    })
    // Browser Back already changed the URL before React hears it. Capture
    // runs first: cancel puts this page back; confirm lets the leave proceed.
    const stay = window.location.pathname + window.location.search
    const onPop = () => {
      if (!dirtyRef.current) return
      if (window.confirm(message)) {
        dirtyRef.current = false
        return
      }
      window.history.pushState({}, '', stay)
    }
    window.addEventListener('popstate', onPop, true)
    return () => {
      setNavGuard(null)
      window.removeEventListener('popstate', onPop, true)
    }
  }, [message])

  useEffect(() => {
    if (!dirty) return
    const onBefore = (e: BeforeUnloadEvent) => {
      e.preventDefault()
      e.returnValue = ''
    }
    window.addEventListener('beforeunload', onBefore)
    return () => window.removeEventListener('beforeunload', onBefore)
  }, [dirty])

  return markClean
}
