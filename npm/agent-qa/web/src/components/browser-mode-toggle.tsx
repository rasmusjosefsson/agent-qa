export function BrowserModeToggle({
  headed,
  onChange,
  disabled = false,
}: {
  headed: boolean
  onChange: (headed: boolean) => void
  disabled?: boolean
}) {
  const headless = !headed
  return (
    <button
      type="button"
      role="switch"
      aria-label="Headless"
      aria-checked={headless}
      disabled={disabled}
      onClick={() => onChange(!headed)}
      title={
        headless
          ? 'Headless is on. The replay runs without a browser window.'
          : 'Headless is off. The replay opens a visible browser window.'
      }
      className="flex h-8 shrink-0 items-center gap-2 whitespace-nowrap rounded-lg border border-border bg-card px-2.5 text-xs font-medium text-foreground shadow-xs transition-colors hover:border-primary/30 disabled:cursor-not-allowed disabled:opacity-50"
    >
      <span>Headless</span>
      <span
        aria-hidden="true"
        className={
          'relative h-4 w-7 shrink-0 rounded-full transition-colors ' +
          (headless ? 'bg-primary' : 'bg-muted-foreground/30')
        }
      >
        <span
          className={
            'absolute left-0.5 top-0.5 size-3 rounded-full bg-primary-foreground transition-transform ' +
            (headless ? 'translate-x-3' : 'translate-x-0')
          }
        />
      </span>
    </button>
  )
}
