/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** `http` (default) talks to the report server. `fixture` reads a static tree. */
  readonly VITE_AQA_SOURCE?: string
  /** Base URL for fixture mode. Defaults to `/fixtures`. */
  readonly VITE_AQA_FIXTURE_BASE?: string
}
