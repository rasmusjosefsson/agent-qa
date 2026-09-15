import { useCallback, useEffect, useState } from "react"

export type Route = { pathname: string; search: string }

function currentRoute(): Route {
  return {
    pathname: window.location.pathname,
    search: window.location.search,
  }
}

/** Client-side navigation — pushState + notify listeners, no full reload. */
export function navigate(to: string) {
  window.history.pushState({}, "", to)
  window.dispatchEvent(new PopStateEvent("popstate"))
}

/** Replace the URL without pushing history (e.g. consuming one-shot params). */
export function replaceRoute(to: string) {
  window.history.replaceState({}, "", to)
  window.dispatchEvent(new PopStateEvent("popstate"))
}

/** Reactive view of the current location; re-renders on push/replace/pop. */
export function useRoute(): Route {
  const [route, setRoute] = useState<Route>(currentRoute)
  useEffect(() => {
    const onChange = () => setRoute(currentRoute())
    window.addEventListener("popstate", onChange)
    return () => window.removeEventListener("popstate", onChange)
  }, [])
  return route
}

/**
 * Click handler for in-app anchors: plain left-click navigates via pushState
 * (no document reload, so the sidebar/theme state survives), while modified
 * clicks (cmd/ctrl/shift/middle-button) keep native new-tab behaviour.
 */
export function useSpaLink(to: string) {
  return useCallback(
    (e: React.MouseEvent) => {
      if (e.defaultPrevented) return
      if (e.button !== 0) return
      if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return
      e.preventDefault()
      if (window.location.pathname + window.location.search !== to) navigate(to)
    },
    [to]
  )
}
