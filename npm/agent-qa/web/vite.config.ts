import path from "path"
import react from "@vitejs/plugin-react"
import tailwindcss from "@tailwindcss/vite"
import { defineConfig, type Plugin } from "vite"

// Set the theme class on <html> BEFORE first paint so a stored dark preference
// doesn't flash light (the ThemeProvider only applies after React mounts). Runs
// on every MPA entry, mirroring the provider's key ('agentqa-theme') + default
// ('light') + system resolution.
function themeInit(): Plugin {
  const body =
    "(function(){try{var k='agentqa-theme',t=localStorage.getItem(k)||'light';" +
    "if(t==='system'){t=matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light';}" +
    "var r=document.documentElement;r.classList.remove('light','dark');r.classList.add(t);}catch(e){}})();"
  return {
    name: "aqa-theme-init",
    transformIndexHtml() {
      return [{ tag: "script", injectTo: "head-prepend", children: body }]
    },
  }
}

// Flat single app, multi-page (Runs / Editor / Chat). Built into lib/public/
// and served by the report-server at /, /editor, /chat (assets under /assets/*).
export default defineConfig({
  plugins: [react(), tailwindcss(), themeInit()],
  resolve: { alias: { "@": path.resolve(__dirname, "./src") } },
  base: "/", // hashed assets resolve under /assets/*
  build: {
    outDir: "../lib/public", // the canonical UI (committed build output)
    // vanilla files are removed manually in the cutover; don't wipe artifacts.
    emptyOutDir: false,
    rollupOptions: {
      input: {
        index: path.resolve(__dirname, "index.html"),
        editor: path.resolve(__dirname, "editor.html"),
        chat: path.resolve(__dirname, "chat.html"),
        cases: path.resolve(__dirname, "cases.html"),
        sets: path.resolve(__dirname, "sets.html"),
        plans: path.resolve(__dirname, "plans.html"),
        personas: path.resolve(__dirname, "personas.html"),
        environments: path.resolve(__dirname, "environments.html"),
        knowledge: path.resolve(__dirname, "knowledge.html"),
        plugins: path.resolve(__dirname, "plugins.html"),
      },
    },
  },
  server: {
    // `npm run dev` (HMR) proxies the API to a running `agent-qa web`.
    proxy: { "/api": "http://127.0.0.1:7878" },
  },
})
