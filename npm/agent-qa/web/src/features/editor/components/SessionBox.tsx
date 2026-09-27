// web/src/features/editor/components/SessionBox.tsx
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import type { BufferState } from '../types'

export function SessionBox({
  buffer,
  scenarios,
  onStart,
  onOpen,
  onFlush,
  onCancel,
  onTogglePause,
  onCheck,
  checkRunning,
  checkReport,
  onDismissCheckReport,
}: {
  buffer: BufferState
  scenarios: string[]
  onStart: (intent: string, url: string) => void
  onOpen: (sid: string) => void
  onFlush: () => void
  onCancel: () => void
  onTogglePause: () => void
  onCheck: () => void
  checkRunning: boolean
  checkReport: string | null
  onDismissCheckReport: () => void
}) {
  const [intent, setIntent] = useState('')
  const [url, setUrl] = useState('')
  const [sid, setSid] = useState('')
  const [confirmOpen, setConfirmOpen] = useState(false)
  const active = !!buffer.sid
  const stepCount = buffer.rows.length

  if (active) {
    return (
      <div className="flex flex-col gap-2 border-b border-border p-3">
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          {buffer.editing ? 'editing' : 'recording'}{' '}
          <code className="rounded bg-muted px-1 py-0.5 font-mono text-foreground">{buffer.sid}</code>
          {buffer.paused && (
            <span className="rounded bg-amber-500/15 px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide text-amber-400">
              paused
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          <Button size="sm" onClick={onFlush}>
            Flush → scenario.json
          </Button>
          <Button
            size="sm"
            variant="outline"
            onClick={onCheck}
            disabled={checkRunning}
            title="Validate the buffer as the scenario flush would write (schema + lint)"
          >
            {checkRunning ? 'Checking…' : 'Check'}
          </Button>
          <Button
            size="sm"
            variant="outline"
            onClick={onTogglePause}
            title={
              buffer.paused
                ? 'Resume capturing steps'
                : 'Pause capture — drive the browser without recording steps'
            }
          >
            {buffer.paused ? 'Resume' : 'Pause'}
          </Button>
          <AlertDialog open={confirmOpen} onOpenChange={setConfirmOpen}>
            <Button size="sm" variant="ghost" onClick={() => setConfirmOpen(true)} title="Discard this recording">
              Cancel
            </Button>
            <AlertDialogContent>
              <AlertDialogHeader>
                <AlertDialogTitle>Discard this recording?</AlertDialogTitle>
                <AlertDialogDescription>
                  {stepCount > 0
                    ? `${stepCount} ${buffer.editing ? 'unsaved edit' : 'recorded step'}${stepCount === 1 ? '' : 's'} will be thrown away. This can’t be undone.`
                    : 'This recording session will be discarded.'}
                </AlertDialogDescription>
              </AlertDialogHeader>
              <AlertDialogFooter>
                <AlertDialogCancel>Cancel</AlertDialogCancel>
                <AlertDialogAction
                  className="bg-destructive text-white hover:bg-destructive/90"
                  onClick={onCancel}
                >
                  Discard
                </AlertDialogAction>
              </AlertDialogFooter>
            </AlertDialogContent>
          </AlertDialog>
        </div>
        {checkReport && (
          <div className="relative rounded border border-border bg-muted/40 px-2 py-1.5">
            <button
              type="button"
              aria-label="Dismiss check report"
              onClick={onDismissCheckReport}
              className="absolute right-1 top-1 grid h-4 w-4 place-items-center rounded text-muted-foreground hover:text-foreground"
            >
              ×
            </button>
            <pre className="max-h-44 overflow-auto whitespace-pre-wrap pr-4 font-mono text-[11px] leading-snug text-foreground">
              {checkReport}
            </pre>
          </div>
        )}
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-3 border-b border-border p-3">
      <div className="flex flex-col gap-1">
        <Label htmlFor="intentInput" className="text-xs">
          Intent
        </Label>
        <Input
          id="intentInput"
          value={intent}
          onChange={(e) => setIntent(e.target.value)}
          placeholder="describe the scenario…"
        />
      </div>
      <div className="flex flex-col gap-1">
        <Label htmlFor="openUrlInput" className="text-xs">
          Open URL (optional)
        </Label>
        <Input
          id="openUrlInput"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          placeholder="https://example.com/"
        />
      </div>
      <Button onClick={() => onStart(intent, url)}>Start recording session</Button>
      {scenarios.length > 0 && (
        <div className="flex items-center gap-2 border-t border-border pt-3">
          <Select value={sid} onValueChange={setSid}>
            <SelectTrigger size="sm" className="h-8 flex-1 text-xs" aria-label="Saved scenario">
              <SelectValue placeholder="saved scenario…" />
            </SelectTrigger>
            <SelectContent>
              {scenarios.map((s) => (
                <SelectItem key={s} value={s}>
                  {s}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button
            size="sm"
            variant="outline"
            disabled={!sid}
            onClick={() => onOpen(sid)}
            title="Load this scenario's steps into the buffer for editing"
          >
            Open to edit
          </Button>
        </div>
      )}
    </div>
  )
}

export default SessionBox
