# Changelog

All notable changes to agent-qa are documented here. This project follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Initial development. The v0.0 series is pre-release — the API and on-disk
shapes are usable for daily work but may change before `1.0.0`.

### Added

- **Recording** — `start`, `record-step`, `fill-unique`, `smart-click`,
  `truncate`, `flush`, `verify`.
- **Replay** — `replay` with profile/session binding, parameter overrides,
  and heal-from-run; per-step ARIA snapshot + screenshot evidence.
- **Compare** — per-step ARIA snapshot diff + screenshot pixel diff.
- **Heal** — `heal-respond`, `heal-promote`, `heal-apply`.
- **Profiles** — `profile-add`, `profile-status`, `profile-bootstrap`,
  `profile-list`.
- **Diagnostics** — `doctor`, `info`, `byo-doctor`, `perf-snapshot`.
- **Plugin protocol** — subprocess + JSON-over-stdio; discovery via
  `--plugin`, `agent-qa.toml`, `AGENT_QA_PLUGINS`, or `$PATH`.
