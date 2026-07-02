import { cn } from "@/lib/utils"

// Lightweight "agent is working" indicator: three soft dots that fade and lift
// in a staggered wave — the familiar "thinking" cadence, not an audio meter.
// Dependency-free (pure CSS — see the `aqa-dot` keyframe in index.css); uses
// the theme's primary color so it tracks light/dark.
export function WorkingIndicator({ className }: { className?: string }) {
  return (
    <span
      className={cn("inline-flex items-center gap-1", className)}
      role="status"
      aria-label="Working"
    >
      {[0, 1, 2].map((i) => (
        <span
          key={i}
          className="aqa-dot size-1.5 rounded-full bg-primary"
          style={{ animationDelay: `${i * 180}ms` }}
        />
      ))}
    </span>
  )
}

export default WorkingIndicator
