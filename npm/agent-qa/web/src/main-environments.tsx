import { StrictMode } from "react"
import { createRoot } from "react-dom/client"

import "./index.css"
import { ThemeProvider } from "./components/theme-provider"
import { AppShell } from "./AppShell"
import EnvironmentsPage from "./features/environments/EnvironmentsPage"

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ThemeProvider defaultTheme="dark" storageKey="agentqa-theme">
      <AppShell tab="environments">
        <EnvironmentsPage />
      </AppShell>
    </ThemeProvider>
  </StrictMode>
)
