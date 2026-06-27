import { Button } from "@/components/ui/button"
import type { ReactNode } from "react"

export type Tab = "runs" | "editor" | "chat"

const TABS: { id: Tab; label: string; href: string }[] = [
  { id: "runs", label: "Runs", href: "/" },
  { id: "editor", label: "Editor", href: "/editor" },
  { id: "chat", label: "Chat", href: "/chat" },
]

// App shell: topbar + tab nav, shared across the three entry points
// (main-runs.tsx / main-editor.tsx / main-chat.tsx). Each tab passes its page
// as children; the placeholder only shows if an entry forgets to.
export function AppShell({ tab, children }: { tab: Tab; children?: ReactNode }) {
  const active = TABS.find((t) => t.id === tab)!

  return (
    <div className="flex h-svh flex-col overflow-hidden bg-background text-foreground">
      <header className="flex h-13 shrink-0 items-stretch gap-5 border-b border-border px-5">
        <div className="flex items-center text-sm font-semibold tracking-tight">agent-qa</div>
        <nav className="flex items-stretch gap-5">
          {TABS.map((t) => (
            <a
              key={t.id}
              href={t.href}
              className={
                "-mb-px flex items-center border-b-2 text-sm transition-colors " +
                (t.id === tab
                  ? "border-foreground text-foreground"
                  : "border-transparent text-muted-foreground hover:text-foreground")
              }
            >
              {t.label}
            </a>
          ))}
        </nav>
      </header>

      {children ? (
        <main className="flex min-h-0 flex-1 flex-col">{children}</main>
      ) : (
        <main className="flex flex-1 items-center justify-center p-8">
          <div className="flex max-w-md flex-col items-start gap-4 rounded-xl border border-border bg-card p-8 text-card-foreground">
            <h1 className="text-lg font-semibold">{active.label}</h1>
            <p className="text-sm leading-relaxed text-muted-foreground">
              {active.label} tab.
            </p>
            <div className="flex flex-wrap gap-2">
              <Button>Primary</Button>
              <Button variant="outline">Outline</Button>
              <Button variant="ghost">Ghost</Button>
              <Button variant="secondary">Secondary</Button>
            </div>
            <p className="font-mono text-xs text-muted-foreground">
              press <kbd>d</kbd> to toggle theme
            </p>
          </div>
        </main>
      )}
    </div>
  )
}
