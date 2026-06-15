# Releasing

Releases are cut by tagging a commit on `main` with a semver tag. The
`release` GitHub Action handles cross-compile, per-platform npm package
publish, and umbrella publish.

## Cutting a release

```bash
# 1. Make sure main is green
gh run list --workflow=ci.yml --branch=main --limit=1

# 2. Tag and push
git tag v0.0.1
git push origin v0.0.1
```

GitHub Actions then runs `.github/workflows/release.yml`:

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

Publish provenance is enabled (`--provenance --access public`); the workflow
runs with `id-token: write` permission for OIDC.

## Required secrets

- `NPM_TOKEN` — npm automation token with publish permissions for
  `agent-qa` and every `agent-qa-<platform>` package. Set in the repo's
  GitHub Actions secrets.

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

The `smoke-install` job in `ci.yml` builds for the current Linux runner,
stages a local platform package, generates the umbrella with a `file:` ref,
`npm pack`s both, installs into a temp dir, and runs `agent-qa skills list`
+ `agent-qa skills get core`. Use the same `scripts/build-platform-pkg.js`
+ `scripts/build-umbrella-pkg.js --local-platforms <plat>` flow for local
testing on macOS / Windows.
