import { StrictMode } from "react"
import { createRoot } from "react-dom/client"

import "./index.css"
import { ThemeProvider } from "./components/theme-provider"
import { AppShell } from "./AppShell"
import EditorPage from "./features/editor/EditorPage"

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ThemeProvider defaultTheme="light" storageKey="agentqa-theme">
      <AppShell tab="editor">
        <EditorPage />
      </AppShell>
    </ThemeProvider>
  </StrictMode>
)
