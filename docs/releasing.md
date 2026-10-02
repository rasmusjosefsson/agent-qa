# Releasing

Releases are cut by tagging a commit on `main` with a semver tag. The
`release` GitHub Action handles cross-compile, per-platform npm package
publish, umbrella publish, and the GitHub release whose notes come from
`CHANGELOG.md`.

## Cutting a release

```bash
# 1. Write entries under `## [Unreleased]` in CHANGELOG.md (Added /
#    Changed / Fixed / Removed — Keep a Changelog style).

# 2. Make sure main is green
gh run list --workflow=ci.yml --branch=main --limit=1

# 3. Prep + cut the release (bump / stamp / commit / tag / push)
node scripts/cut-release.js patch --push       # or minor / major / x.y.z
```

`scripts/cut-release.js` on a clean, up-to-date `main`:

- computes the next version (or takes an explicit `x.y.z`),
- moves the `## [Unreleased]` body under a dated `## [<v>] - <date>`
  heading and bumps `npm/agent-qa/package.json` to the same version,
- commits `chore(release): v<v>` and tags `v<v>`,
- prints the push command; `--push` pushes `main` + the tag (which is
  what actually ships). `--dry-run` previews without touching files or
  git.

It bails when nothing is under `## [Unreleased]` (pass `--allow-empty`
for a mechanical bump) and when `## [<v>]` already exists — in which
case the changelog/version are left as-is and only the tag + push
happen. That pre-prepped path is how a release whose changelog section
and package version were landed earlier (like `v0.1.0`) gets tagged.

The tag push runs `.github/workflows/release.yml`:

1. **Stamps `cli/Cargo.toml`'s `[package].version` from the tag** so
   the compiled binary's `agent-qa --version` matches the npm package
   version. `cli/Cargo.lock` is dropped before build so it regenerates
   against the new version.
2. Cross-compiles the Rust binary for each supported platform tuple
   (`darwin-arm64`, `darwin-x64`, `linux-x64`, `win32-x64`).
3. Stages each platform's npm package directory under
   `npm/platform/<platform>/` via `scripts/build-platform-pkg.js`.
4. Publishes each platform package (`agent-qa-<platform>`) to npm with
   matching version.
5. Stages the umbrella `agent-qa` package via
   `scripts/build-umbrella-pkg.js` (sets `version` and aligns every
   `optionalDependencies` entry to the same version).
6. Publishes the umbrella package.
7. Creates the GitHub release for the tag, extracting the `## [<v>]`
   section of `CHANGELOG.md` as the release notes (a missing section
   falls back to a pointer at the changelog).

Publish provenance is enabled (`--provenance --access public`); the workflow
runs with `id-token: write` permission for OIDC.

## Who releases

Releases are cut by the repository owner only. The npm credentials live
in the owner's GitHub Actions secrets; contributors do not publish.

## First release prerequisites

The first time each `agent-qa-<platform>` is published it MUST be available
under your npm account / org. Either reserve them ahead of time with
`npm publish` from a stub, or accept that the first `release` workflow run
creates them.

## Adding more platforms

The matrix in `release.yml` and the `PLATFORMS` table in
`scripts/build-platform-pkg.js` are the two places to update.
`linux-arm64`, `linux-musl-x64`, `linux-musl-arm64` are the obvious next
candidates; they need `cross` or `cargo-zigbuild` to cross-compile from the
Linux runner. Add them as separate matrix entries with the right `target`
and toolchain setup, then add corresponding entries to the umbrella's
`optionalDependencies`.

## Local smoke test

The `smoke-install` job in `ci.yml` builds on each matrix runner (macOS and
Linux), stages a local platform package, generates the umbrella with a `file:` ref,
`npm pack`s both, installs into a temp dir, and runs `agent-qa skills list`
+ `agent-qa skills get core`. Use the same `scripts/build-platform-pkg.js`
+ `scripts/build-umbrella-pkg.js --local-platforms <plat>` flow for local
testing on macOS / Windows.
