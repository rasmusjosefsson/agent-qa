import { useState } from 'react'
import { BookOpenIcon, DownloadIcon, Loader2Icon } from 'lucide-react'
import { PageHeader } from '@/components/page-header'
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
import { navigate } from '@/router'
import { caseIdFromKey, setIdFromKey, type XrayContainer } from './importPrompt'

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
        blurb: 'Import a test plan, set, story, or epic. Fetches the tests read-only and creates cases + a set.',
        tile: 'bg-emerald-600',
        status: 'import-xray',
      },
      {
        key: 'jira',
        name: 'Jira',
        blurb: 'Import a single issue as a test case, read-only.',
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
  const [error, setError] = useState('')
  const [job, setJob] = useState<{ id: string; phase?: string; current?: number; total?: number } | null>(null)

  const runImport = async (body: Record<string, string>) => {
    setBusy(true)
    setError('')
    setJob(null)
    try {
      const res = await fetch('/api/knowledge/import', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(body),
      })
      const data = await res.json()
      if (!res.ok) throw new Error(data.error || `import failed (${res.status})`)
      const jobId: string = data.job.id
      // Poll until the job settles — each case lands as it's imported, so a
      // cancel still keeps everything that finished.
      for (;;) {
        await new Promise((r) => setTimeout(r, 1500))
        const jr = await fetch(`/api/knowledge/import/${jobId}`)
        const { job: j } = await jr.json()
        setJob(j)
        if (j.status === 'done' || j.status === 'canceled' || j.status === 'failed') {
          if (j.status === 'failed') throw new Error(j.error || j.result?.error || 'import failed')
          if (j.result?.warnings?.length) console.warn('[knowledge import]', j.result.warnings)
          setJiraOpen(false)
          setXrayOpen(false)
          navigate(j.result?.set ? '/sets' : '/cases')
          return
        }
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setBusy(false)
      setJob(null)
    }
  }

  const cancelImport = () => {
    if (job) void fetch(`/api/knowledge/import/${job.id}/cancel`, { method: 'POST' })
  }
  const importJira = () => {
    const k = key.trim()
    if (k) void runImport({ source: 'jira', key: k })
  }
  const importXray = () => {
    const k = key.trim()
    if (k) void runImport({ source: 'xray', key: k, container })
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
      <PageHeader
        icon={BookOpenIcon}
        title="Knowledge"
        description="Connect the tools where your tests and product knowledge already live."
      />

      <div className="min-h-0 flex-1 space-y-8 overflow-auto px-5 py-5">
        {GROUPS.map((g) => (
          <section key={g.label}>
            <h2 className="text-sm font-semibold tracking-tight text-foreground">{g.label}</h2>
            <p className="mb-3 text-xs text-muted-foreground">{g.hint}</p>
            <div className="aqa-table-card aqa-elevated">
              {g.items.map((s) => (
                <div
                  key={s.key}
                  className="flex items-center gap-3 border-b border-border/60 px-4 py-3 transition-colors last:border-0 hover:bg-accent/40"
                >
                  <div
                    className={cn(
                      'grid size-9 shrink-0 place-items-center rounded-lg text-sm font-semibold text-white shadow-sm',
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
              Enter an issue key. Its fields and Xray steps are fetched read-only and saved as a local test case you can run.
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
            {error && <p className="text-xs text-destructive">{error}</p>}
          </div>
          {busy && (
            <p className="text-xs text-muted-foreground">
              {job?.total ? `${job.current ?? 0}/${job.total} — ` : ''}{job?.phase || 'starting…'}
            </p>
          )}
          <DialogFooter>
            {busy ? (
              <Button variant="ghost" onClick={cancelImport}>
                Stop import
              </Button>
            ) : (
              <Button variant="ghost" onClick={() => setJiraOpen(false)}>
                Cancel
              </Button>
            )}
            <Button onClick={importJira} disabled={!key.trim() || busy}>
              {busy && <Loader2Icon className="animate-spin" />} Import
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={xrayOpen} onOpenChange={setXrayOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Import from Xray</DialogTitle>
            <DialogDescription>
              Pick what to import and its key. Each test becomes a local case, grouped into a set.
              Read-only — nothing is written back to Jira.
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
              {error && <p className="text-xs text-destructive">{error}</p>}
            </div>
          </div>
          {busy && (
            <p className="text-xs text-muted-foreground">
              {job?.total ? `${job.current ?? 0}/${job.total} — ` : ''}{job?.phase || 'starting…'}
            </p>
          )}
          <DialogFooter>
            {busy ? (
              <Button variant="ghost" onClick={cancelImport}>
                Stop import
              </Button>
            ) : (
              <Button variant="ghost" onClick={() => setXrayOpen(false)}>
                Cancel
              </Button>
            )}
            <Button onClick={importXray} disabled={!key.trim() || busy}>
              {busy && <Loader2Icon className="animate-spin" />} Import
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}

export default KnowledgePage
