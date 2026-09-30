import type { ComponentType, ReactNode, SVGProps } from "react"
import { cn } from "@/lib/utils"

type Icon = ComponentType<SVGProps<SVGSVGElement>>

// Shared page header: gradient icon tile, title, one-line explainer, actions
// on the right, and an optional slot underneath (e.g. the test pipeline).
export function PageHeader({
  icon: Icon,
  title,
  description,
  actions,
  children,
  className,
}: {
  icon?: Icon
  title: ReactNode
  description?: ReactNode
  actions?: ReactNode
  children?: ReactNode
  className?: string
}) {
  return (
    <div className={cn("relative shrink-0 border-b border-border", className)}>
      <div className="flex flex-col gap-3 px-6 pt-5 pb-4 md:flex-row md:items-center md:justify-between md:gap-6">
        <div className="flex min-w-0 flex-1 items-center gap-3.5">
          {Icon && (
            <div className="grid size-10 shrink-0 place-items-center rounded-xl bg-primary/10 text-primary">
              <Icon className="size-5" />
            </div>
          )}
          <div className="min-w-0">
            <h1 className="text-xl font-semibold tracking-tight text-foreground">{title}</h1>
            {description && (
              <p className="mt-0.5 max-w-3xl text-[13px] leading-relaxed text-muted-foreground">
                {description}
              </p>
            )}
          </div>
        </div>
        {actions && <div className="flex shrink-0 items-center gap-1.5">{actions}</div>}
      </div>
      {children && <div className="px-6 pb-4">{children}</div>}
    </div>
  )
}

// Inline error banner that sits under a PageHeader.
export function PageError({ children }: { children: ReactNode }) {
  return (
    <div className="flex items-center gap-2 border-b border-destructive/25 bg-destructive/8 px-6 py-2 text-xs text-destructive">
      <span className="size-1.5 shrink-0 rounded-full bg-destructive" />
      {children}
    </div>
  )
}
