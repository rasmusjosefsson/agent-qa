import { StrictMode } from "react"
import { createRoot } from "react-dom/client"

import "./index.css"
import { ThemeProvider } from "./components/theme-provider"
import { AppShell } from "./AppShell"
import SourcesPage from "./features/sources/SourcesPage"

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ThemeProvider defaultTheme="dark" storageKey="agentqa-theme">
      <AppShell tab="sources">
        <SourcesPage />
      </AppShell>
    </ThemeProvider>
  </StrictMode>
)
