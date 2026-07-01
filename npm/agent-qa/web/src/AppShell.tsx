import type { ReactNode } from "react"
import { AppSidebar } from "@/components/app-sidebar"
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
} from "@/components/ui/sidebar"
import { Separator } from "@/components/ui/separator"
import { TooltipProvider } from "@/components/ui/tooltip"
import { ThemeToggle } from "@/components/theme-toggle"

export type Tab =
  | "cases"
  | "sets"
  | "plans"
  | "runs"
  | "editor"
  | "chat"
  | "personas"
  | "environments"
  | "knowledge"
  | "plugins"

const LABELS: Record<Tab, string> = {
  cases: "Test Cases",
  sets: "Test Sets",
  plans: "Test Plans",
  runs: "Test Runs",
  editor: "Editor",
  chat: "Copilot",
  personas: "Personas",
  environments: "Environments",
  knowledge: "Knowledge",
  plugins: "Plugins",
}

// App shell: collapsible sidebar (shadcn sidebar-07) + a thin topbar, shared
// across the MPA entries (main-cases / main-sets / main-plans / main-runs /
// main-editor / main-chat / main-knowledge). Each entry passes its tab; the
// sidebar highlights it and the
// page renders as children. Collapse state persists across full-page nav via
// the sidebar's `sidebar_state` cookie (path=/).
export function AppShell({ tab, children }: { tab: Tab; children?: ReactNode }) {
  return (
    <TooltipProvider delayDuration={0}>
      <SidebarProvider className="h-svh min-h-svh overflow-hidden bg-background text-foreground">
        <AppSidebar tab={tab} />
        <SidebarInset className="flex min-h-0 min-w-0 flex-col overflow-hidden">
        <header className="flex h-13 shrink-0 items-center gap-2 border-b border-border px-3">
          <SidebarTrigger className="-ml-1" />
          <Separator orientation="vertical" className="mr-1 h-4" />
          <span className="text-sm font-semibold tracking-tight">{LABELS[tab]}</span>
          <div className="ml-auto">
            <ThemeToggle />
          </div>
        </header>
        {children ? (
          <div className="flex min-h-0 flex-1 flex-col">{children}</div>
        ) : (
          <div className="flex flex-1 items-center justify-center p-8 text-sm text-muted-foreground">
            {LABELS[tab]}
          </div>
        )}
        </SidebarInset>
      </SidebarProvider>
    </TooltipProvider>
  )
}
