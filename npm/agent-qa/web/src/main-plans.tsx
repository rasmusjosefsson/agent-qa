import { StrictMode } from "react"
import { createRoot } from "react-dom/client"

import "./index.css"
import { ThemeProvider } from "./components/theme-provider"
import { AppShell } from "./AppShell"
import PlansPage from "./features/plans/PlansPage"

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ThemeProvider defaultTheme="dark" storageKey="agentqa-theme">
      <AppShell tab="plans">
        <PlansPage />
      </AppShell>
    </ThemeProvider>
  </StrictMode>
)
