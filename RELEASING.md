# Releasing gproxy (giant-proxy)

## Cut a release

```bash
git push origin main          # push code first
./release.sh 0.7.0
```

`release.sh` does:
1. Bumps version in `crates/giantd/Cargo.toml`, `crates/giant-proxy/Cargo.toml`, `crates/giant-proxy-ui/src-tauri/Cargo.toml`
2. Bumps version in `crates/giant-proxy-ui/src-tauri/tauri.conf.json`
3. Runs `cargo generate-lockfile`
4. Commits `bump to v0.7.0`, tags `v0.7.0`, pushes to `origin`

## What happens after push

```
v0.7.0 tag pushed
  → release.yml build-cli: builds gproxy + giantd, tars → gproxy-v0.7.0-aarch64-apple-darwin.tar.gz
  → release.yml build-tauri: builds .app + .dmg → Giant.Proxy_0.7.0_aarch64.dmg
  → release job uploads tarballs to release; tauri uploaded by tauri-action
  → release:published event triggers update-homebrew.yml
  → downloads tarball + DMG, computes SHAs
  → regenerates Formula/giant-proxy-cli.rb + Casks/giant-proxy.rb
  → commits to bearded-giant/homebrew-tap as github-actions[bot]
```

## Watch

```bash
gh run watch -R bearded-giant/gproxy
gh run list -R bearded-giant/gproxy --workflow=update-homebrew.yml --limit 1
```

## Verify install

```bash
brew update
brew upgrade giant-proxy-cli      # CLI + daemon only
brew upgrade --cask giant-proxy   # app + CLI + daemon
```

## Secrets

| Secret | Required for |
|---|---|
| `GITHUB_TOKEN` | auto |
| `HOMEBREW_TAP_TOKEN` | update-homebrew.yml push to tap |
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | tauri code-signing + notarization |

Set tap token:
```bash
gh secret set HOMEBREW_TAP_TOKEN -R bearded-giant/gproxy --body "$HOMEBREW_TAP_TOKEN"
```

## Manual backfill

```bash
gh workflow run update-homebrew.yml -R bearded-giant/gproxy -f tag=v0.7.0
```

## Failure recovery

| Symptom | Fix |
|---|---|
| release.yml red | fix on main, `git tag -d v0.7.0 && git push origin :v0.7.0 && ./release.sh 0.7.0` |
| Apple signing fails | `APPLE_*` secrets expired. Re-export, update via `gh secret set` |
| Tap formula stale | `gh workflow run update-homebrew.yml -R bearded-giant/gproxy -f tag=v0.7.0` |
| Asset pattern mismatch | update patterns in `update-homebrew.yml`: CLI `gproxy-v${VERSION}-aarch64-apple-darwin.tar.gz`, DMG `Giant.Proxy_${VERSION}_aarch64.dmg` |

See also: tap-wide `~/dev/homebrew-tap/RELEASING.md`.
