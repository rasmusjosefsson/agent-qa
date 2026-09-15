import path from "path"
import react from "@vitejs/plugin-react"
import tailwindcss from "@tailwindcss/vite"
import { defineConfig, type Plugin } from "vite"

// Set the theme class on <html> BEFORE first paint so a stored dark preference
// doesn't flash light (the ThemeProvider only applies after React mounts).
// Mirrors the provider's key ('agentqa-theme') + default ('light') + system
// resolution.
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

// Single-page app: one index.html entry, client-side routing across all tabs.
// Built into lib/public/ and served by the report-server at / + every tab
// route (assets under /assets/*).
export default defineConfig({
  plugins: [react(), tailwindcss(), themeInit()],
  resolve: { alias: { "@": path.resolve(__dirname, "./src") } },
  base: "/", // hashed assets resolve under /assets/*
  build: {
    outDir: "../lib/public", // the canonical UI (committed build output)
    emptyOutDir: true,
    rollupOptions: {
      input: {
        index: path.resolve(__dirname, "index.html"),
      },
    },
  },
  server: {
    // `npm run dev` (HMR) proxies the API to a running `agent-qa web`.
    proxy: { "/api": "http://127.0.0.1:7878" },
  },
})
