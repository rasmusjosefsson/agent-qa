// Step-verb badge colours, shared by the editor's step list and the Runs
// timeline so a "click" or "check" reads the same everywhere. Each pairs a
// light-theme and dark-theme text shade over a translucent fill.
export const VERB_TONE: Record<string, string> = {
  nav: 'bg-info/12 text-info',
  click: 'bg-primary/12 text-primary',
  fill: 'bg-teal-500/12 text-teal-700 dark:text-teal-300',
  press: 'bg-violet-500/12 text-violet-600 dark:text-violet-300',
  assert: 'bg-success/12 text-success',
  wait: 'bg-muted text-muted-foreground',
  action: 'bg-muted text-muted-foreground',
}
