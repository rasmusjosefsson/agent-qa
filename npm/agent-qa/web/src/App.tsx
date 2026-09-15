import { AppShell, type Tab } from "./AppShell"
import { useRoute } from "./router"
import RunsPage from "./features/runs/RunsPage"
import CasesPage from "./features/cases/CasesPage"
import SetsPage from "./features/sets/SetsPage"
import PlansPage from "./features/plans/PlansPage"
import EditorPage from "./features/editor/EditorPage"
import ChatPage from "./features/chat/ChatPage"
import PersonasPage from "./features/personas/PersonasPage"
import EnvironmentsPage from "./features/environments/EnvironmentsPage"
import KnowledgePage from "./features/knowledge/KnowledgePage"
import PluginsPage from "./features/plugins/PluginsPage"

function tabForPath(pathname: string): Tab {
  switch (pathname) {
    case "/cases":
    case "/cases.html":
      return "cases"
    case "/sets":
    case "/sets.html":
      return "sets"
    case "/plans":
    case "/plans.html":
      return "plans"
    case "/editor":
    case "/editor.html":
      return "editor"
    case "/chat":
    case "/chat.html":
      return "chat"
    case "/personas":
    case "/personas.html":
      return "personas"
    case "/environments":
    case "/environments.html":
      return "environments"
    case "/knowledge":
    case "/knowledge.html":
      return "knowledge"
    case "/plugins":
    case "/plugins.html":
      return "plugins"
    case "/":
    case "/index.html":
    default:
      return "runs"
  }
}

// Single-page app root: one persistent AppShell (sidebar + theme survive),
// page content swaps without a document reload. Query strings stay on the
// route (list/detail views read them reactively) and key the content wrapper
// so list ↔ detail still gets a soft enter transition instead of a flash.
export function App() {
  const route = useRoute()
  const tab = tabForPath(route.pathname)

  return (
    <AppShell tab={tab}>
      <div key={`${tab}${route.search}`} className="aqa-page-enter flex min-h-0 flex-1 flex-col">
        {tab === "runs" && <RunsPage key={route.search} />}
        {tab === "cases" && <CasesPage />}
        {tab === "sets" && <SetsPage />}
        {tab === "plans" && <PlansPage />}
        {tab === "editor" && <EditorPage />}
        {tab === "chat" && <ChatPage />}
        {tab === "personas" && <PersonasPage />}
        {tab === "environments" && <EnvironmentsPage />}
        {tab === "knowledge" && <KnowledgePage />}
        {tab === "plugins" && <PluginsPage />}
      </div>
    </AppShell>
  )
}

export default App
