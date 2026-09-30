import type { AnchorHTMLAttributes, ReactNode } from "react"
import { useSpaLink } from "@/router"

// In-app anchor: plain click pushes state (no document reload); modified
// clicks keep native new-tab behaviour. Forwards everything Radix Slot merges
// onto it (className for active / hover pills, data attributes, …).
export function SpaLink({
  href,
  children,
  ...rest
}: {
  href: string
  children: ReactNode
} & AnchorHTMLAttributes<HTMLAnchorElement>) {
  const onClick = useSpaLink(href)
  return (
    <a href={href} {...rest} onClick={onClick}>
      {children}
    </a>
  )
}
