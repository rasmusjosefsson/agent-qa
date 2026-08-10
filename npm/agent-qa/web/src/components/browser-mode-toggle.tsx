import { MonitorIcon } from 'lucide-react'

export function BrowserModeToggle({
  headed,
  onChange,
  disabled = false,
}: {
  headed: boolean
  onChange: (headed: boolean) => void
  disabled?: boolean
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-label="Show browser window"
      aria-checked={headed}
      disabled={disabled}
      onClick={() => onChange(!headed)}
      title="Headless by default. Changing mode restarts the selected browser session."
      className="flex h-7 items-center gap-1.5 rounded-md border border-border bg-background px-2 text-xs text-foreground transition-colors hover:bg-muted disabled:cursor-not-allowed disabled:opacity-50"
    >
      <MonitorIcon className="size-3.5" />
      <span>{headed ? 'Browser visible' : 'Headless'}</span>
      <span
        aria-hidden="true"
        className={
          'relative h-4 w-7 rounded-full transition-colors ' +
          (headed ? 'bg-emerald-500/80' : 'bg-muted-foreground/30')
        }
      >
        <span
          className={
            'absolute top-0.5 size-3 rounded-full bg-white transition-transform ' +
            (headed ? 'translate-x-3.5' : 'translate-x-0.5')
          }
        />
      </span>
    </button>
  )
}
