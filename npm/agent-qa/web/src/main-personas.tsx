import { StrictMode } from "react"
import { createRoot } from "react-dom/client"

import "./index.css"
import { ThemeProvider } from "./components/theme-provider"
import { AppShell } from "./AppShell"
import PersonasPage from "./features/personas/PersonasPage"

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ThemeProvider defaultTheme="dark" storageKey="agentqa-theme">
      <AppShell tab="personas">
        <PersonasPage />
      </AppShell>
    </ThemeProvider>
  </StrictMode>
)
