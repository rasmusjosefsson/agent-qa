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
  UsersIcon,
} from "lucide-react"

import type { Tab } from "../AppShell"
import { cn } from "@/lib/utils"
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

// The agent-qa "spark" mark — same shield + spark as the site's logo.svg,
// inlined so the workbench brand matches the docs. The tile is dark on light
// sidebars and light on dark (mirroring logo.svg / logo-light.svg).
function AgentSparkMark({ className }: { className?: string }) {
  const variants = [
    { tile: '#12121f', gradIds: ['aqa-wm', 'aqa-wm-b'], cls: 'dark:hidden' },
    { tile: '#f4f3fb', gradIds: ['aqa-wm-l', 'aqa-wm-lb'], cls: 'hidden dark:block' },
  ]
  return (
    <>
      {variants.map((v) => (
        <svg key={v.tile} viewBox="0 0 64 64" fill="none" className={cn(className, v.cls)} aria-hidden>
          <defs>
            <linearGradient id={v.gradIds[0]} x1="10" y1="10" x2="54" y2="54" gradientUnits="userSpaceOnUse">
              <stop stopColor="#22d3ee" />
              <stop offset="1" stopColor="#8b5cf6" />
            </linearGradient>
            <linearGradient id={v.gradIds[1]} x1="20" y1="16" x2="44" y2="48" gradientUnits="userSpaceOnUse">
              <stop stopColor="#a78bfa" />
              <stop offset="1" stopColor="#f472b6" />
            </linearGradient>
          </defs>
          <rect width="64" height="64" rx="16" fill={v.tile} />
          <path
            d="M32 10.5 49 17v13.5c0 11-7.3 19.5-17 23-9.7-3.5-17-12-17-23V17Z"
            stroke={`url(#${v.gradIds[0]})`}
            strokeWidth="3.6"
            strokeLinejoin="round"
          />
          <path
            d="M32 19.5c1.1 6.2 3.6 8.9 9.5 10-5.9 1.1-8.4 3.8-9.5 10.5-1.1-6.7-3.6-9.4-9.5-10.5 5.9-1.1 8.4-3.8 9.5-10Z"
            fill={`url(#${v.gradIds[1]})`}
          />
        </svg>
      ))}
    </>
  )
}

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
                <AgentSparkMark className="size-8 shrink-0 rounded-lg" />
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
