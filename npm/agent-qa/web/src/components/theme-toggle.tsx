import { MonitorIcon, MoonIcon, SunIcon } from "lucide-react"
import { useTheme } from "@/components/theme-provider"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"

// System / Light / Dark selector for the topbar. The choice persists via
// ThemeProvider (localStorage `agentqa-theme`) and is applied before first
// paint by the Vite theme-init script, so it survives reloads without a flash.
// The trigger icon reflects the *resolved* theme (system → OS preference).
export function ThemeToggle() {
  const { theme, setTheme } = useTheme()
  const prefersDark =
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-color-scheme: dark)").matches
  const isDark = theme === "dark" || (theme === "system" && prefersDark)
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="icon" className="size-8" aria-label="Theme">
          {isDark ? <MoonIcon className="size-4" /> : <SunIcon className="size-4" />}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuRadioGroup
          value={theme}
          onValueChange={(v) => setTheme(v as "system" | "light" | "dark")}
        >
          <DropdownMenuRadioItem value="system">
            <MonitorIcon className="size-4" /> System
          </DropdownMenuRadioItem>
          <DropdownMenuRadioItem value="light">
            <SunIcon className="size-4" /> Light
          </DropdownMenuRadioItem>
          <DropdownMenuRadioItem value="dark">
            <MoonIcon className="size-4" /> Dark
          </DropdownMenuRadioItem>
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

export default ThemeToggle
