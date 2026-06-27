// web/src/features/editor/components/Composer.tsx
import { Button } from '@/components/ui/button'
import { CheckIcon, XIcon } from 'lucide-react'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { cn } from '@/lib/utils'
import { VERB_FIELDS, VERB_OPTIONS } from '../compose'
import type { ComposeForm, RunResult } from '../types'

export function Composer({
  form,
  onField,
  pickedHint,
  onRun,
  onRecord,
  runResult,
}: {
  form: ComposeForm
  onField: (field: keyof ComposeForm, value: string) => void
  pickedHint: string
  onRun: () => void
  onRecord: () => void
  runResult: RunResult | null
}) {
  const spec = VERB_FIELDS[form.verb] || { fields: [] }
  const show = new Set(spec.fields)
  const valueLabel = spec.value || 'Value'
  const nameLabel = spec.name || 'Name / label'

  return (
    <div className="flex flex-col gap-3 p-3">
      <div className="text-xs font-medium text-muted-foreground">Add a step</div>

      <div className="flex flex-col gap-1">
        <Label className="text-xs">Step</Label>
        <Select value={form.verb} onValueChange={(v) => onField('verb', v)}>
          <SelectTrigger size="sm" className="w-full text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {VERB_OPTIONS.map((o) => (
              <SelectItem key={o.value} value={o.value}>
                {o.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {show.has('role') && (
        <Field label="Role" value={form.role} placeholder="button" onChange={(v) => onField('role', v)} />
      )}
      {show.has('name') && (
        <Field label={nameLabel} value={form.name} placeholder="Login" onChange={(v) => onField('name', v)} />
      )}
      {show.has('value') && (
        <Field label={valueLabel} value={form.value} placeholder="" onChange={(v) => onField('value', v)} />
      )}
      {show.has('intent') && (
        <Field
          label="Intent (assert contract)"
          value={form.intent}
          placeholder="why this step matters"
          onChange={(v) => onField('intent', v)}
        />
      )}

      {pickedHint && <div className="text-xs text-muted-foreground">{pickedHint}</div>}

      <div className="flex items-center gap-2">
        <Button size="sm" variant="outline" onClick={onRun}>
          Run step (live)
        </Button>
        <Button size="sm" onClick={onRecord}>
          Record step
        </Button>
      </div>

      {runResult && (
        <div
          className={cn(
            'flex items-center gap-1.5 rounded-md px-2 py-1.5 text-xs',
            runResult.recorded || runResult.ok
              ? 'bg-emerald-500/15 text-emerald-400'
              : 'bg-destructive/15 text-destructive'
          )}
        >
          {runResult.recorded || runResult.ok ? (
            <>
              <CheckIcon className="size-3.5 shrink-0" />
              {runResult.recorded ? 'recorded' : 'step passed'}
            </>
          ) : (
            <>
              <XIcon className="size-3.5 shrink-0" />
              {runResult.error || 'step failed'}
            </>
          )}
        </div>
      )}
    </div>
  )
}

function Field({
  label,
  value,
  placeholder,
  onChange,
}: {
  label: string
  value: string
  placeholder: string
  onChange: (v: string) => void
}) {
  return (
    <div className="flex flex-col gap-1">
      <Label className="text-xs">{label}</Label>
      <Input value={value} placeholder={placeholder} onChange={(e) => onChange(e.target.value)} className="text-xs" />
    </div>
  )
}

export default Composer
