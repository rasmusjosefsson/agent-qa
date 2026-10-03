import type { ComponentType, ReactNode, SVGProps } from "react"
import {
  BookOpenIcon,
  BotIcon,
  CirclePlayIcon,
  ClipboardListIcon,
  FolderTreeIcon,
  GlobeIcon,
  ImagesIcon,
  LayersIcon,
  PlugIcon,
  Settings2Icon,
  SquarePenIcon,
  UsersIcon,
} from "lucide-react"
import { AppSidebar } from "@/components/app-sidebar"
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
} from "@/components/ui/sidebar"
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
  | "goldens"
  | "settings"

const LABELS: Record<Tab, string> = {
  cases: "Test Cases",
  sets: "Test Sets",
  plans: "Test Plans",
  runs: "Test Runs",
  editor: "Editor",
  chat: "Chat",
  personas: "Personas",
  environments: "Environments",
  knowledge: "Knowledge",
  plugins: "Extensions",
  goldens: "Goldens",
  settings: "Settings",
}

const ICONS: Record<Tab, ComponentType<SVGProps<SVGSVGElement>>> = {
  cases: ClipboardListIcon,
  sets: LayersIcon,
  plans: FolderTreeIcon,
  runs: CirclePlayIcon,
  editor: SquarePenIcon,
  chat: BotIcon,
  personas: UsersIcon,
  environments: GlobeIcon,
  knowledge: BookOpenIcon,
  plugins: PlugIcon,
  goldens: ImagesIcon,
  settings: Settings2Icon,
}

const GROUP: Record<Tab, string> = {
  chat: "Create",
  editor: "Create",
  cases: "Test pipeline",
  sets: "Test pipeline",
  plans: "Test pipeline",
  runs: "Test pipeline",
  personas: "Setup",
  environments: "Setup",
  knowledge: "Setup",
  plugins: "Setup",
  goldens: "Test pipeline",
  settings: "Workspace",
}

// App shell: collapsible sidebar (shadcn sidebar-07) + a thin topbar, mounted
// once for the whole SPA session. Page content swaps inside without a document
// reload, so the sidebar / theme / scroll state survive navigation.
export function AppShell({ tab, children }: { tab: Tab; children?: ReactNode }) {
  const Icon = ICONS[tab]
  return (
    <TooltipProvider delayDuration={0}>
      <SidebarProvider className="h-svh min-h-svh overflow-hidden bg-background text-foreground">
        <AppSidebar tab={tab} />
        <SidebarInset className="flex min-h-0 min-w-0 flex-col overflow-hidden">
        <header className="relative z-20 flex h-12 shrink-0 items-center gap-2 border-b border-border bg-background/75 px-3 backdrop-blur-xl backdrop-saturate-150">
          <SidebarTrigger className="text-muted-foreground hover:text-foreground" />
          <div className="mx-1 h-4 w-px bg-border" />
          <span className="hidden text-[13px] text-muted-foreground sm:inline">{GROUP[tab]}</span>
          <span className="hidden text-[13px] text-muted-foreground/40 sm:inline">/</span>
          <span className="flex items-center gap-1.5 text-[13px] font-semibold tracking-tight text-foreground">
            <Icon className="size-3.5 text-primary" />
            {LABELS[tab]}
          </span>
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
