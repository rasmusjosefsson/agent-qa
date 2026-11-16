import { LOCAL_ONLY_NOTE } from '@/lib/source'

/** A workbench page the static demo cannot run. No /api calls. */
export function DemoLocalOnly({ label }: { label: string }) {
  return (
    <div className="grid min-h-0 flex-1 place-items-center p-8" data-testid="demo-local-only">
      <div className="max-w-sm text-center">
        <h1 className="text-lg font-semibold text-foreground">{label}</h1>
        <p className="mt-2 text-sm text-muted-foreground">{LOCAL_ONLY_NOTE}</p>
      </div>
    </div>
  )
}
