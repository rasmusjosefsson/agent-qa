// web/src/features/editor/components/SessionBox.tsx
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
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
  onStart,
  onFlush,
  onCancel,
}: {
  buffer: BufferState
  onStart: (intent: string, url: string) => void
  onFlush: () => void
  onCancel: () => void
}) {
  const [intent, setIntent] = useState('')
  const [url, setUrl] = useState('')
  const [confirmOpen, setConfirmOpen] = useState(false)
  const active = !!buffer.sid
  const stepCount = buffer.rows.length

  if (active) {
    return (
      <div className="flex flex-col gap-2 rounded-lg border border-border bg-card p-3">
        <div className="text-xs text-muted-foreground">
          recording <code className="rounded bg-muted px-1 py-0.5 font-mono text-foreground">{buffer.sid}</code>
        </div>
        <div className="flex items-center gap-2">
          <Button size="sm" onClick={onFlush}>
            Flush → scenario.json
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
                    ? `${stepCount} recorded step${stepCount === 1 ? '' : 's'} will be thrown away. This can’t be undone.`
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
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-3 rounded-lg border border-border bg-card p-3">
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
    </div>
  )
}

export default SessionBox
