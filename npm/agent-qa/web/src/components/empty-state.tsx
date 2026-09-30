import type { ComponentType, ReactNode, SVGProps } from "react"
import { cn } from "@/lib/utils"

type Icon = ComponentType<SVGProps<SVGSVGElement>>

// Centered empty state: haloed icon tile, title, explainer, and a CTA.
export function EmptyState({
  icon: Icon,
  title,
  description,
  action,
  className,
}: {
  icon: Icon
  title: ReactNode
  description?: ReactNode
  action?: ReactNode
  className?: string
}) {
  return (
    <div className={cn("flex h-full min-h-72 flex-col items-center justify-center gap-4 p-8 text-center", className)}>
      <div className="grid size-12 place-items-center rounded-2xl bg-primary/10 text-primary">
        <Icon className="size-6" />
      </div>
      <div>
        <div className="text-base font-semibold tracking-tight text-foreground">{title}</div>
        {description && (
          <p className="mx-auto mt-1.5 max-w-sm text-[13px] leading-relaxed text-muted-foreground">{description}</p>
        )}
      </div>
      {action}
    </div>
  )
}

// Centered "loading …" line used while a list fetches.
export function LoadingState({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full min-h-48 items-center justify-center gap-2.5 text-sm text-muted-foreground">
      <span className="relative flex size-2">
        <span className="aqa-ping size-2 rounded-full bg-primary text-primary" />
      </span>
      {children}
    </div>
  )
}
