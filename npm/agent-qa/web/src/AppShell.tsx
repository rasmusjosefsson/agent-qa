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
  chat: "Copilot QA",
  personas: "Personas",
  environments: "Environments",
  knowledge: "Knowledge",
  plugins: "Extensions",
}

// App shell: collapsible sidebar (shadcn sidebar-07) + a thin topbar, mounted
// once for the whole SPA session. Page content swaps inside without a document
// reload, so the sidebar / theme / scroll state survive navigation.
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
