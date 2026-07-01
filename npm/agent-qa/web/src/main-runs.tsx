import { StrictMode } from "react"
import { createRoot } from "react-dom/client"

import "./index.css"
import { ThemeProvider } from "./components/theme-provider"
import { AppShell } from "./AppShell"
import RunsPage from "./features/runs/RunsPage"

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ThemeProvider defaultTheme="light" storageKey="agentqa-theme">
      <AppShell tab="runs">
        <RunsPage />
      </AppShell>
    </ThemeProvider>
  </StrictMode>
)
