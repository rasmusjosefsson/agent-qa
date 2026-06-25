// web/src/features/chat/components/PromptInput.tsx
import { useEffect, useRef, type KeyboardEvent } from 'react'
import type { ModelInfo } from '@/lib/types'
import { Button } from '@/components/ui/button'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'

function modelKey(m?: ModelInfo) {
  return m ? (m.provider ? m.provider + '/' : '') + m.id : ''
}

export interface PromptInputProps {
  value: string
  onChange: (v: string) => void
  onSubmit: () => void
  onAbort: () => void
  available: boolean
  streaming: boolean
  models: ModelInfo[]
  model?: ModelInfo
  onModel: (provider?: string, id?: string) => void
  thinkingLevel?: string
  thinkingLevels: string[]
  onThinking: (level: string) => void
}

export function PromptInput(props: PromptInputProps) {
  const {
    value,
    onChange,
    onSubmit,
    onAbort,
    available,
    streaming,
    models,
    model,
    onModel,
    thinkingLevel,
    thinkingLevels,
    onThinking,
  } = props
  const taRef = useRef<HTMLTextAreaElement | null>(null)

  // Auto-grow the textarea (capped), mirroring classic autoGrow().
  useEffect(() => {
    const ta = taRef.current
    if (!ta) return
    ta.style.height = 'auto'
    ta.style.height = Math.min(ta.scrollHeight, 220) + 'px'
  }, [value])

  function onKeyDown(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      onSubmit()
    }
  }

  return (
    <div className="rounded-xl border border-border bg-card">
      <textarea
        ref={taRef}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={onKeyDown}
        rows={1}
        disabled={!available}
        placeholder={available ? 'Message the agent…  (Enter to send, Shift+Enter for newline)' : 'Agent unavailable'}
        className="w-full resize-none bg-transparent px-3.5 py-3 text-sm outline-none placeholder:text-muted-foreground disabled:opacity-60"
      />
      <div className="flex items-center justify-between gap-2 border-t border-border px-2.5 py-2">
        <div className="flex items-center gap-2">
          {models.length > 0 && (
            <Select
              value={modelKey(model)}
              disabled={!available}
              onValueChange={(val) => {
                const slash = val.indexOf('/')
                const provider = slash >= 0 ? val.slice(0, slash) : undefined
                const id = slash >= 0 ? val.slice(slash + 1) : val
                onModel(provider, id)
              }}
            >
              <SelectTrigger size="sm" className="h-8 text-xs" aria-label="Model">
                <SelectValue placeholder="Model" />
              </SelectTrigger>
              <SelectContent>
                {models.map((m) => (
                  <SelectItem key={modelKey(m)} value={modelKey(m)}>
                    {m.label || m.id}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          )}

          {thinkingLevels.length >= 2 && (
            <Select
              value={thinkingLevel || ''}
              disabled={!available}
              onValueChange={(val) => onThinking(val)}
            >
              <SelectTrigger size="sm" className="h-8 text-xs" aria-label="Thinking level">
                <SelectValue placeholder="Thinking" />
              </SelectTrigger>
              <SelectContent>
                {thinkingLevels.map((lvl) => (
                  <SelectItem key={lvl} value={lvl}>
                    {lvl === 'off' ? 'no thinking' : 'think: ' + lvl}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          )}
        </div>

        <div className="flex items-center gap-2">
          {streaming && (
            <Button type="button" variant="destructive" size="sm" onClick={onAbort}>
              Stop
            </Button>
          )}
          <Button type="button" size="sm" onClick={onSubmit} disabled={!available || !value.trim()}>
            Send
          </Button>
        </div>
      </div>
    </div>
  )
}

export default PromptInput
