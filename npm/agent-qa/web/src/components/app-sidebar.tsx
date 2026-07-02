import { useEffect, useState, type ComponentProps, type ComponentType, type SVGProps } from "react"
import {
  BookOpenIcon,
  ClipboardListIcon,
  CirclePlayIcon,
  FolderTreeIcon,
  GlobeIcon,
  LayersIcon,
  PlugIcon,
  Settings2Icon,
  SparklesIcon,
  SquarePenIcon,
  TestTubeDiagonalIcon,
  UsersIcon,
} from "lucide-react"

import type { Tab } from "../AppShell"
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
  { label: "Copilot QA", icon: SparklesIcon, tab: "chat", href: "/chat" },
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

const WORKSPACE: NavItem[] = [{ label: "Settings", icon: Settings2Icon, soon: true }]

function NavRow({ item, tab }: { item: NavItem; tab: Tab }) {
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
      <SidebarMenuButton asChild isActive={item.tab === tab} tooltip={item.label}>
        <a href={item.href}>
          <Icon />
          <span>{item.label}</span>
        </a>
      </SidebarMenuButton>
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
              <a href="/cases">
                <div className="flex aspect-square size-8 items-center justify-center rounded-lg bg-sidebar-primary text-sidebar-primary-foreground">
                  <TestTubeDiagonalIcon className="size-4" />
                </div>
                <div className="grid flex-1 text-left text-sm leading-tight">
                  <span className="truncate font-semibold">agent-qa</span>
                  <span className="truncate text-xs text-muted-foreground">
                    QA workbench{version ? ` · v${version}` : ''}
                  </span>
                </div>
              </a>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup>
          <SidebarMenu>
            {AUTHORING.map((item) => (
              <NavRow key={item.label} item={item} tab={tab} />
            ))}
          </SidebarMenu>
        </SidebarGroup>

        <SidebarGroup>
          <SidebarGroupLabel>Tests</SidebarGroupLabel>
          <SidebarMenu>
            {TESTS.map((item) => (
              <NavRow key={item.label} item={item} tab={tab} />
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
