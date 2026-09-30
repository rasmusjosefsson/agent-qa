import { useEffect, useState, type ComponentProps, type ComponentType, type SVGProps } from "react"
import {
  BookOpenIcon,
  BotIcon,
  SparklesIcon,
  ClipboardListIcon,
  CirclePlayIcon,
  FolderTreeIcon,
  GlobeIcon,
  LayersIcon,
  PlugIcon,
  Settings2Icon,
  SquarePenIcon,
  TestTubeDiagonalIcon,
  UsersIcon,
} from "lucide-react"

import type { Tab } from "../AppShell"
import { SpaLink as SpaAnchor } from "@/components/spa-link"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
} from "@/components/ui/sidebar"

type Icon = ComponentType<SVGProps<SVGSVGElement>>

type NavItem = {
  label: string
  icon: Icon
  tab?: Tab // present ⇒ real, navigable section
  href?: string
  soon?: boolean
}

// Authoring — the two ways you create/drive a test: talk to the Copilot QA
// agent, or hand-edit a recorded scenario. Kept at the top as the entry points.
const AUTHORING: NavItem[] = [
  { label: "Chat", icon: BotIcon, tab: "chat", href: "/chat" },
  { label: "Editor", icon: SquarePenIcon, tab: "editor", href: "/editor" },
]

// Tests — the case → set → plan → run pipeline. The group header carries the
// "Test" context, so the items drop the redundant prefix.
const TESTS: NavItem[] = [
  { label: "Cases", icon: ClipboardListIcon, tab: "cases", href: "/cases" },
  { label: "Sets", icon: LayersIcon, tab: "sets", href: "/sets" },
  { label: "Plans", icon: FolderTreeIcon, tab: "plans", href: "/plans" },
  { label: "Runs", icon: CirclePlayIcon, tab: "runs", href: "/" },
]

// Setup — identities, targets, knowledge, and extension packages.
const SETUP: NavItem[] = [
  { label: "Personas", icon: UsersIcon, tab: "personas", href: "/personas" },
  { label: "Environments", icon: GlobeIcon, tab: "environments", href: "/environments" },
  { label: "Knowledge", icon: BookOpenIcon, tab: "knowledge", href: "/knowledge" },
  { label: "Extensions", icon: PlugIcon, tab: "plugins", href: "/plugins" },
]

const WORKSPACE: NavItem[] = [{ label: "Settings", icon: Settings2Icon, tab: "settings", href: "/settings" }]

function NavRow({ item, tab, step }: { item: NavItem; tab: Tab; step?: number }) {
  const Icon = item.icon
  if (item.soon || !item.href) {
    return (
      <SidebarMenuItem>
        <SidebarMenuButton disabled tooltip={`${item.label} — coming soon`}>
          <Icon />
          <span>{item.label}</span>
        </SidebarMenuButton>
        <SidebarMenuBadge className="text-[10px] uppercase tracking-wide opacity-60">
          Soon
        </SidebarMenuBadge>
      </SidebarMenuItem>
    )
  }
  return (
    <SidebarMenuItem>
      <SidebarMenuButton
        asChild
        isActive={item.tab === tab}
        tooltip={item.label}
      >
        {/* SPA link: plain click pushes state (no document reload); modified
            clicks keep native new-tab behaviour. */}
        <SpaAnchor href={item.href}>
          <Icon />
          <span>{item.label}</span>
        </SpaAnchor>
      </SidebarMenuButton>
      {step !== undefined && (
        <SidebarMenuBadge
          className={
            item.tab === tab
              ? "rounded-md bg-sidebar-primary/12 font-mono text-[10px] text-sidebar-primary"
              : "rounded-md font-mono text-[10px] text-sidebar-foreground/35"
          }
        >
          {step}
        </SidebarMenuBadge>
      )}
    </SidebarMenuItem>
  )
}

export function AppSidebar({ tab, ...props }: { tab: Tab } & ComponentProps<typeof Sidebar>) {
  // Installed agent-qa version, shown under the logo (e.g. "QA workbench · v0.0.42").
  const [version, setVersion] = useState<string>('')
  useEffect(() => {
    fetch('/api/version', { headers: { accept: 'application/json' } })
      .then((r) => (r.ok ? r.json() : null))
      .then((j) => j && typeof j.version === 'string' && setVersion(j.version))
      .catch(() => {})
  }, [])
  return (
    <Sidebar collapsible="icon" {...props}>
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton asChild size="lg" tooltip={version ? `agent-qa v${version}` : 'agent-qa'}>
              <SpaAnchor href="/cases">
                <div className="flex aspect-square size-8 items-center justify-center rounded-lg bg-primary text-primary-foreground">
                  <TestTubeDiagonalIcon className="size-4" />
                </div>
                <div className="grid flex-1 text-left text-sm leading-tight">
                  <span className="truncate font-semibold tracking-tight text-foreground">agent-qa</span>
                  <span data-qa-volatile className="truncate text-xs text-muted-foreground">
                    QA workbench{version ? ` · v${version}` : ''}
                  </span>
                </div>
              </SpaAnchor>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupLabel>Create</SidebarGroupLabel>
          <SidebarMenu>
            {AUTHORING.map((item) => (
              <NavRow key={item.label} item={item} tab={tab} />
            ))}
          </SidebarMenu>
        </SidebarGroup>

        <SidebarGroup>
          <SidebarGroupLabel>Test pipeline</SidebarGroupLabel>
          <SidebarMenu>
            {TESTS.map((item, i) => (
              <NavRow key={item.label} item={item} tab={tab} step={i + 1} />
            ))}
          </SidebarMenu>
        </SidebarGroup>

        <SidebarGroup>
          <SidebarGroupLabel>Setup</SidebarGroupLabel>
          <SidebarMenu>
            {SETUP.map((item) => (
              <NavRow key={item.label} item={item} tab={tab} />
            ))}
          </SidebarMenu>
        </SidebarGroup>
      </SidebarContent>

      <SidebarFooter>
        <SpaAnchor
          href="/chat"
          className="group/tip mx-1 mb-1 block overflow-hidden rounded-xl border border-sidebar-border bg-card/60 p-3 transition-colors hover:border-primary/30 group-data-[collapsible=icon]:hidden"
        >
          <div className="-m-3 mb-0 p-3 pb-2">
            <div className="flex items-center gap-1.5 text-xs font-semibold text-foreground">
              <SparklesIcon className="size-3.5 text-primary" /> New here?
            </div>
          </div>
          <p className="text-[11.5px] leading-relaxed text-muted-foreground">
            Describe a test in <span className="font-medium text-foreground">Chat</span>. The agent records it, then you
            replay it from <span className="font-medium text-foreground">Runs</span>.
          </p>
        </SpaAnchor>
        <SidebarMenu>
          {WORKSPACE.map((item) => (
            <NavRow key={item.label} item={item} tab={tab} />
          ))}
        </SidebarMenu>
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  )
}
