import { useCallback, useEffect, useState } from "react"

export type Route = { pathname: string; search: string }

function currentRoute(): Route {
  return {
    pathname: window.location.pathname,
    search: window.location.search,
  }
}

function here(): string {
  return window.location.pathname + window.location.search
}

// In-app navigations (sidebar links, `navigate()`) ask this before leaving.
// Return false to stay put — used for unsaved editors. One guard at a time;
// the mounted page owns it and clears it on unmount.
let navGuard: (() => boolean) | null = null

export function setNavGuard(next: (() => boolean) | null) {
  navGuard = next
}

function allowLeave(): boolean {
  if (!navGuard) return true
  try {
    return navGuard() !== false
  } catch {
    return true
  }
}

/** Fired after a committed in-app navigation, including a same-page link click. */
export const NAV_EVENT = "aqa-navigate"

function announceNavigate() {
  window.dispatchEvent(new Event(NAV_EVENT))
}

/** Client-side navigation — pushState + notify listeners, no full reload. */
export function navigate(to: string) {
  if (here() === to) {
    announceNavigate()
    return
  }
  if (!allowLeave()) return
  window.history.pushState({}, "", to)
  window.dispatchEvent(new PopStateEvent("popstate"))
  announceNavigate()
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
      navigate(to)
    },
    [to]
  )
}
