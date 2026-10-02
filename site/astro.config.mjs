import starlight from "@astrojs/starlight"
import { defineConfig } from "astro/config"

// GitHub Pages project site. Override both for a custom domain.
const site = process.env.SITE_URL ?? "https://rasmusjosefsson.github.io"
const base = process.env.SITE_BASE ?? "/agent-qa"

export default defineConfig({
  site,
  base,
  trailingSlash: "always",
  integrations: [
    starlight({
      title: "agent-qa",
      description: "Record a user scenario in a real browser. Replay it later. See exactly what changed.",
      logo: { dark: "./src/assets/logo.svg", light: "./src/assets/logo-light.svg" },
      favicon: "/favicon.svg",
      social: [{ icon: "github", label: "GitHub", href: "https://github.com/rasmusjosefsson/agent-qa" }],
      customCss: [
        "@fontsource-variable/geist",
        "@fontsource-variable/geist-mono",
        "./src/styles/tokens.css",
        "./src/styles/starlight.css",
        "./src/styles/diagrams.css",
      ],
      components: { Head: "./src/components/DocsHead.astro", PageTitle: "./src/components/PageTitle.astro" },
      expressiveCode: {
        themes: ["github-dark-default", "github-light-default"],
        styleOverrides: { borderRadius: "0.75rem", codeFontFamily: "var(--aqa-font-mono)" },
      },
      sidebar: [
        {
          label: "Start here",
          items: [
            { label: "Quickstart", slug: "docs/quickstart" },
            { label: "How it works", slug: "docs/overview" },
          ],
        },
        {
          label: "Guides",
          items: [
            { label: "Configuration", slug: "docs/configuration" },
            { label: "Plugins", slug: "docs/plugins" },
            { label: "Templates", slug: "docs/templates" },
            { label: "Golden screenshots", slug: "docs/visual-testing" },
            { label: "Network claims", slug: "docs/network" },
            { label: "Browser extension", slug: "docs/extension" },
            { label: "GitHub Action", slug: "docs/github-action" },
            { label: "Session hygiene", slug: "docs/process-hygiene" },
          ],
        },
        {
          label: "Reference",
          items: [
            { label: "CLI verbs", slug: "docs/verbs" },
            { label: "Lint rules", slug: "docs/lint-rules" },
            { label: "Architecture", slug: "docs/architecture" },
          ],
        },
        { label: "Contributing", items: ["docs/releasing"] },
      ],
    }),
  ],
})
