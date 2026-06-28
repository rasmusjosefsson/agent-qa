import type { ComponentProps, ComponentType, SVGProps } from "react"
import {
  BookOpenIcon,
  ClipboardListIcon,
  CirclePlayIcon,
  FolderTreeIcon,
  GlobeIcon,
  LayersIcon,
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

// Working sections (each maps to an MPA entry / tab). "Copilot" is the chat
// agent; "Knowledge" is the connector hub (import/integrations).
const PROJECT: NavItem[] = [
  { label: "Test Cases", icon: ClipboardListIcon, tab: "cases", href: "/cases" },
  { label: "Test Sets", icon: LayersIcon, tab: "sets", href: "/sets" },
  { label: "Test Plans", icon: FolderTreeIcon, tab: "plans", href: "/plans" },
  { label: "Test Runs", icon: CirclePlayIcon, tab: "runs", href: "/" },
  { label: "Editor", icon: SquarePenIcon, tab: "editor", href: "/editor" },
  { label: "Copilot", icon: SparklesIcon, tab: "chat", href: "/chat" },
  { label: "Knowledge", icon: BookOpenIcon, tab: "knowledge", href: "/knowledge" },
]

// Standard QA objects we don't cover yet — shown disabled so the model is
// legible. Wired up as they land.
const SOON: NavItem[] = [
  { label: "Environments", icon: GlobeIcon, soon: true },
  { label: "Personas", icon: UsersIcon, soon: true },
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
  return (
    <Sidebar collapsible="icon" {...props}>
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton asChild size="lg" tooltip="agent-qa">
              <a href="/cases">
                <div className="flex aspect-square size-8 items-center justify-center rounded-lg bg-sidebar-primary text-sidebar-primary-foreground">
                  <TestTubeDiagonalIcon className="size-4" />
                </div>
                <div className="grid flex-1 text-left text-sm leading-tight">
                  <span className="truncate font-semibold">agent-qa</span>
                  <span className="truncate text-xs text-muted-foreground">QA workbench</span>
                </div>
              </a>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupLabel>Project</SidebarGroupLabel>
          <SidebarMenu>
            {PROJECT.map((item) => (
              <NavRow key={item.label} item={item} tab={tab} />
            ))}
          </SidebarMenu>
        </SidebarGroup>

        <SidebarGroup>
          <SidebarGroupLabel>Coming soon</SidebarGroupLabel>
          <SidebarMenu>
            {SOON.map((item) => (
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
