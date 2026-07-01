import { cn } from "@/lib/utils"

// Lightweight "agent is working" indicator: a small row of bars that pulse
// like a waveform, on staggered delays. Dependency-free (pure CSS — see the
// `aqa-wave` keyframe in index.css); uses the theme's primary color so it
// tracks light/dark. Replaces the heavy LiveKit audio-visualizer.
export function WorkingIndicator({ className }: { className?: string }) {
  return (
    <span
      className={cn("inline-flex h-4 items-stretch gap-[3px]", className)}
      role="status"
      aria-label="Working"
    >
      {[0, 1, 2, 3, 4].map((i) => (
        <span
          key={i}
          className="aqa-wave h-full w-[3px] rounded-full bg-primary"
          style={{ animationDelay: `${i * 110}ms` }}
        />
      ))}
    </span>
  )
}

export default WorkingIndicator
