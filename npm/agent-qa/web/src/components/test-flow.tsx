import { ChevronRightIcon, CirclePlayIcon, ClipboardListIcon, FolderTreeIcon, LayersIcon } from "lucide-react"
import { SpaLink } from "@/components/spa-link"
import { cn } from "@/lib/utils"

export type FlowStep = "cases" | "sets" | "plans" | "runs"

const STEPS: { key: FlowStep; label: string; hint: string; href: string; icon: typeof LayersIcon }[] = [
  { key: "cases", label: "Cases", hint: "Write a test in plain English", href: "/cases", icon: ClipboardListIcon },
  { key: "sets", label: "Sets", hint: "Group cases you run together", href: "/sets", icon: LayersIcon },
  { key: "plans", label: "Plans", hint: "Pick a scope and run it", href: "/plans", icon: FolderTreeIcon },
  { key: "runs", label: "Runs", hint: "Replay, inspect and compare", href: "/", icon: CirclePlayIcon },
]

// The case → set → plan → run pipeline, with the current stage lit. Each
// stage links to its page, so the strip doubles as a map of how tests flow.
export function TestFlow({ current, className }: { current: FlowStep; className?: string }) {
  return (
    <ol className={cn("flex items-stretch gap-1 overflow-x-auto no-scrollbar", className)} aria-label="How tests flow">
      {STEPS.map((s, i) => {
        const Icon = s.icon
        const active = s.key === current
        return (
          <li key={s.key} className="flex shrink-0 items-center gap-1">
            <SpaLink
              href={s.href}
              aria-current={active ? "step" : undefined}
              className={cn(
                "group flex items-center gap-2.5 rounded-lg border px-2.5 py-1.5 transition-all",
                active
                  ? "border-primary/30 bg-card"
                  : "border-transparent hover:border-border hover:bg-card/70"
              )}
            >
              <span
                className={cn(
                  "grid size-6 shrink-0 place-items-center rounded-md text-[11px] font-semibold transition-colors",
                  active
                    ? "bg-primary text-primary-foreground"
                    : "bg-muted text-muted-foreground group-hover:bg-accent group-hover:text-accent-foreground"
                )}
              >
                {active ? <Icon className="size-3.5" /> : i + 1}
              </span>
              <span className="min-w-0 leading-tight">
                <span className={cn("block text-xs font-semibold", active ? "text-foreground" : "text-muted-foreground group-hover:text-foreground")}>
                  {s.label}
                </span>
                <span className="hidden text-[11px] whitespace-nowrap text-muted-foreground sm:block">{s.hint}</span>
              </span>
            </SpaLink>
            {i < STEPS.length - 1 && <ChevronRightIcon className="size-3.5 shrink-0 text-muted-foreground/40" />}
          </li>
        )
      })}
    </ol>
  )
}
