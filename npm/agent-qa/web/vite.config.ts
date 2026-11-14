import path from "path"
import react from "@vitejs/plugin-react"
import tailwindcss from "@tailwindcss/vite"
import { defineConfig, type Plugin } from "vite"

// Set the theme class on <html> BEFORE first paint so a stored dark preference
// doesn't flash light (the ThemeProvider only applies after React mounts).
// Mirrors the provider's key ('agentqa-theme') + default ('light') + system
// resolution.
function playHtml(): Plugin {
  return {
    name: "aqa-play-html",
    transformIndexHtml(html) {
      if (process.env.VITE_AQA_SOURCE !== "fixture") return html
      return html
        .replace("<div id=\"root\"></div>", "<main id=\"root\"></main>")
        .replace(
          "</head>",
          '<meta name="aqa-golden-ready" content="[data-demo-ready]" />\n  </head>'
        )
    },
  }
}

function themeInit(): Plugin {
  // The docs demo follows the site theme until the workbench toggle is used.
  // The local app keeps its own key and does not read the docs theme.
  const read =
    process.env.VITE_AQA_SOURCE === "fixture"
      ? "var k='agentqa-theme',t=localStorage.getItem(k)||localStorage.getItem('starlight-theme')||'light';"
      : "var k='agentqa-theme',t=localStorage.getItem(k)||'light';"
  const body =
    "(function(){try{" +
    read +
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
  plugins: [react(), tailwindcss(), themeInit(), playHtml()],
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
    // String-shorthand proxying leaves /api/* hanging until the socket
    // dies when the report-server is down — the UI sat on "loading…"
    // forever. Fail fast with a JSON 502 so callers hit their error path
    // immediately. No socket timeout: the chat browser-stream SSE is
    // long-lived and a timeout would kill healthy streams.
    proxy: {
      "/api": {
        target: "http://127.0.0.1:7878",
        configure: (proxy) => {
          proxy.on("error", (_err, _req, res) => {
            const w = res as unknown as {
              headersSent?: boolean;
              writeHead?: (code: number, h: Record<string, string>) => void;
              end?: (body?: string) => void;
            };
            if (typeof w.writeHead === "function" && !w.headersSent) {
              w.writeHead(502, { "content-type": "application/json" });
              w.end?.(
                JSON.stringify({
                  error:
                    "report-server unreachable — start it with `agent-qa web` (or check the port)",
                })
              );
            } else {
              w.end?.();
            }
          });
        },
      },
    },
  },
})
