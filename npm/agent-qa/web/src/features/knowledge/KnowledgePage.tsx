import { useState } from 'react'
import { DownloadIcon, Loader2Icon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { cn } from '@/lib/utils'
import {
  buildJiraImportPrompt,
  buildXrayImportPrompt,
  caseIdFromKey,
  setIdFromKey,
  type XrayContainer,
} from './importPrompt'

// Connectors are imported through the Copilot agent, which holds the actual
// credentials/MCP access. This hub just frames what's connectable and builds
// the import instruction; `import` = wired today, `soon` = planned.
type Source = {
  key: string
  name: string
  blurb: string
  tile: string // tailwind bg for the lettered tile
  status: 'import-jira' | 'import-xray' | 'soon'
}

const GROUPS: { label: string; hint: string; items: Source[] }[] = [
  {
    label: 'Test management',
    hint: 'Pull existing tests in as cases — grouped into a set.',
    items: [
      {
        key: 'xray',
        name: 'Xray for Jira',
        blurb: 'Import a test plan, set, story, or epic. The Copilot agent fetches the tests and creates cases + a set.',
        tile: 'bg-emerald-600',
        status: 'import-xray',
      },
      {
        key: 'jira',
        name: 'Jira',
        blurb: 'Import a single issue as a test case via your Atlassian connection.',
        tile: 'bg-blue-600',
        status: 'import-jira',
      },
      { key: 'linear', name: 'Linear', blurb: 'Issues and projects.', tile: 'bg-violet-600', status: 'soon' },
      { key: 'github', name: 'GitHub Issues', blurb: 'Issues and pull requests.', tile: 'bg-zinc-700', status: 'soon' },
    ],
  },
  {
    label: 'Product knowledge',
    hint: 'Docs and designs the agent can read for context.',
    items: [
      { key: 'confluence', name: 'Confluence', blurb: 'Wiki pages and specs.', tile: 'bg-sky-700', status: 'soon' },
      { key: 'notion', name: 'Notion', blurb: 'Docs and databases.', tile: 'bg-neutral-800', status: 'soon' },
      { key: 'gdocs', name: 'Google Docs', blurb: 'Documents and notes.', tile: 'bg-blue-500', status: 'soon' },
      { key: 'figma', name: 'Figma', blurb: 'Designs and flows.', tile: 'bg-fuchsia-600', status: 'soon' },
    ],
  },
]

const CONTAINERS: { value: XrayContainer; label: string }[] = [
  { value: 'plan', label: 'Test plan' },
  { value: 'set', label: 'Test set' },
  { value: 'story', label: 'Story' },
  { value: 'epic', label: 'Epic' },
]

export function KnowledgePage() {
  const [jiraOpen, setJiraOpen] = useState(false)
  const [xrayOpen, setXrayOpen] = useState(false)
  const [key, setKey] = useState('')
  const [container, setContainer] = useState<XrayContainer>('plan')
  const [busy, setBusy] = useState(false)

  const toCopilot = (prompt: string) => {
    setBusy(true)
    window.location.href = `/chat?ask=${encodeURIComponent(prompt)}`
  }
  const importJira = () => {
    const k = key.trim()
    if (k) toCopilot(buildJiraImportPrompt(k, window.location.origin))
  }
  const importXray = () => {
    const k = key.trim()
    if (k) toCopilot(buildXrayImportPrompt(k, container, window.location.origin))
  }

  const openJira = () => {
    setKey('')
    setJiraOpen(true)
  }
  const openXray = () => {
    setKey('')
    setContainer('plan')
    setXrayOpen(true)
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="border-b border-border px-5 py-3">
        <h1 className="text-base font-semibold tracking-tight">Knowledge</h1>
        <p className="text-xs text-muted-foreground">
          Connect the tools where your tests and product knowledge already live.
        </p>
      </div>

      <div className="min-h-0 flex-1 space-y-8 overflow-auto px-5 py-5">
        {GROUPS.map((g) => (
          <section key={g.label}>
            <h2 className="text-sm font-medium text-foreground">{g.label}</h2>
            <p className="mb-3 text-xs text-muted-foreground">{g.hint}</p>
            <div className="overflow-hidden rounded-lg border border-border">
              {g.items.map((s) => (
                <div
                  key={s.key}
                  className="flex items-center gap-3 border-b border-border/60 px-4 py-3 last:border-0"
                >
                  <div
                    className={cn(
                      'grid size-9 shrink-0 place-items-center rounded-md text-sm font-semibold text-white',
                      s.tile
                    )}
                  >
                    {s.name[0]}
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="text-sm font-medium text-foreground">{s.name}</div>
                    <div className="truncate text-xs text-muted-foreground">{s.blurb}</div>
                  </div>
                  {s.status === 'import-xray' ? (
                    <Button size="sm" onClick={openXray}>
                      <DownloadIcon /> Import tests
                    </Button>
                  ) : s.status === 'import-jira' ? (
                    <Button size="sm" variant="secondary" onClick={openJira}>
                      <DownloadIcon /> Import issue
                    </Button>
                  ) : (
                    <span className="rounded-full border border-border bg-muted/40 px-2.5 py-1 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
                      Coming soon
                    </span>
                  )}
                </div>
              ))}
            </div>
          </section>
        ))}
      </div>

      <Dialog open={jiraOpen} onOpenChange={setJiraOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Import from Jira</DialogTitle>
            <DialogDescription>
              Enter an issue key. The Copilot agent will fetch it and create a test case you can run.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-2">
            <Label htmlFor="jira-key">Issue key</Label>
            <Input
              id="jira-key"
              autoFocus
              placeholder="e.g. PROJ-123"
              value={key}
              onChange={(e) => setKey(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') importJira()
              }}
            />
            {key.trim() && (
              <p className="font-mono text-[11px] text-muted-foreground">
                new case id: {caseIdFromKey(key)}
              </p>
            )}
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setJiraOpen(false)}>
              Cancel
            </Button>
            <Button onClick={importJira} disabled={!key.trim() || busy}>
              {busy && <Loader2Icon className="animate-spin" />} Import in Copilot
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={xrayOpen} onOpenChange={setXrayOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Import from Xray</DialogTitle>
            <DialogDescription>
              Pick what to import and its key. The Copilot agent fetches the tests and creates a case
              for each, grouped into a set.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            <div className="space-y-2">
              <Label>Import scope</Label>
              <div className="grid grid-cols-4 gap-1.5">
                {CONTAINERS.map((c) => (
                  <button
                    key={c.value}
                    type="button"
                    onClick={() => setContainer(c.value)}
                    className={cn(
                      'rounded-md border px-2 py-1.5 text-xs transition-colors',
                      container === c.value
                        ? 'border-primary bg-primary/10'
                        : 'border-border hover:bg-muted/40'
                    )}
                  >
                    {c.label}
                  </button>
                ))}
              </div>
            </div>
            <div className="space-y-2">
              <Label htmlFor="xray-key">Key</Label>
              <Input
                id="xray-key"
                autoFocus
                placeholder="e.g. PROJ-100"
                value={key}
                onChange={(e) => setKey(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') importXray()
                }}
              />
              {key.trim() && (
                <p className="font-mono text-[11px] text-muted-foreground">
                  new set id: {setIdFromKey(key)}
                </p>
              )}
            </div>
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setXrayOpen(false)}>
              Cancel
            </Button>
            <Button onClick={importXray} disabled={!key.trim() || busy}>
              {busy && <Loader2Icon className="animate-spin" />} Import in Copilot
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}

export default KnowledgePage
