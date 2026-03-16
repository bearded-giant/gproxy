# Giant Proxy

HTTPS proxy with Map Remote rules. Redirect production URLs to local dev servers using named profiles with glob and regex matching. Ships as a daemon + CLI + menubar app.

Giant Proxy sits between your browser and the internet, intercepting HTTPS requests that match your rules and redirecting them to local services. Think of it as a programmable `/etc/hosts` that works with HTTPS, supports path matching, and has a UI.

## Getting Started (macOS, from GitHub Release)

1. Download the `.dmg` from the [latest release](https://github.com/bearded-giant/gproxy/releases/latest)
2. Open the `.dmg` and drag **Giant Proxy** to Applications
3. Launch Giant Proxy -- on first run it will prompt for your password twice:
   - once to install CLI commands (`giant-proxy`, `giantd`) to `/usr/local/bin`
   - once to trust the generated CA certificate in the system keychain
4. Import your Proxyman rules from the Dashboard (Settings > Import from Proxyman) or create a profile manually
5. Click **Start Proxy** from the tray menu or select a profile in the Dashboard and click **Start**

That's it. Everything is bundled in the app -- daemon, CLI, and cert setup. The system proxy is configured automatically when active and restored when stopped. The CLI is available in your terminal immediately after install (`giant-proxy status`, `giant-proxy on --profile preprod`, etc).

## Install

### Homebrew (macOS/Linux)

```
brew install bearded-giant/tap/giant-proxy
brew install --cask bearded-giant/tap/giant-proxy-ui   # optional menubar app
```

### Curl

```
curl -sSL https://raw.githubusercontent.com/bearded-giant/gproxy/main/install.sh | bash
```

### From Source

```
git clone https://github.com/bearded-giant/gproxy.git
cd gproxy
cargo build --release
cp target/release/giantd target/release/giant-proxy ~/.local/bin/
```

## Quick Start

1. Initialize the config directory and generate a CA certificate:

```
giant-proxy init
```

This creates `~/.giant-proxy/` with a CA cert. You'll be prompted for your password to trust the cert in your system keychain.

2. Create a profile at `~/.giant-proxy/profiles/preprod.toml`:

```toml
[meta]
name = "preprod"
description = "Preprod environment redirects"
format_version = 1

[[rules]]
id = "merchant_portal"
enabled = true
preserve_host = true

[rules.match]
host = "*.preprod.example.com"
path = "/merchant/*"
not_path = "/merchant/v1/*"

[rules.target]
host = "localhost"
port = 3000
scheme = "http"
```

3. Start the proxy with a profile:

```
giant-proxy on --profile preprod
```

This starts the daemon, loads the profile, and configures your system proxy automatically. No manual proxy setup or env vars needed.

4. Open `https://store.preprod.example.com/merchant/settings` in your browser -- it hits your local `:3000` instead.

5. When you're done:

```
giant-proxy stop
```

System proxy is restored automatically.

## CLI Reference

| Command | Description |
|---------|-------------|
| `giant-proxy init` | Create config directory, generate CA cert, install to trust store |
| `giant-proxy start` | Start the daemon |
| `giant-proxy stop` | Stop the daemon and restore system proxy |
| `giant-proxy on [--profile NAME]` | Start daemon + load profile + set system proxy (picks first profile if none specified) |
| `giant-proxy off` | Stop matching, clear active profile |
| `giant-proxy status` | Show proxy status, active profile, loaded rules (enabled count) |
| `giant-proxy health` | Full diagnostic: CA cert, trust store, daemon, profiles |
| `giant-proxy env` | Print shell export statements for proxy env vars |
| `giant-proxy profile list` | List available profiles |
| `giant-proxy profile show NAME` | Show profile details and rules |
| `giant-proxy profile create NAME` | Create an empty profile |
| `giant-proxy profile import FILE --all` | Import profiles from Proxyman or legacy format |
| `giant-proxy profile import-proxyman` | Import directly from local Proxyman install |
| `giant-proxy profile export NAME [--format FMT]` | Export profile (formats: toml, mitmproxy) |
| `giant-proxy rule list PROFILE` | List rules in a profile |
| `giant-proxy rule add PROFILE --id ID ...` | Add a rule to a profile |
| `giant-proxy rule toggle PROFILE RULE_ID` | Enable/disable a rule |
| `giant-proxy daemon install` | Install as a system service (launchd/systemd) |
| `giant-proxy daemon uninstall` | Remove the system service |
| `giant-proxy uninstall` | Remove everything: service, CA cert, config directory |
| `giant-proxy version` | Print version |

## Profiles

Profiles are TOML files in `~/.giant-proxy/profiles/`. Each file is a named set of redirect rules. Drop a `.toml` file in that directory and it's available immediately.

### Rule Matching

Each rule has a `[rules.match]` section that determines which requests it intercepts. Rules are evaluated in order; first match wins.

| Field | Type | Description |
|-------|------|-------------|
| `host` | glob | Hostname pattern, e.g. `*.preprod.example.com` |
| `path` | glob | Path pattern, e.g. `/merchant/*` |
| `not_path` | glob | Exclude pattern, e.g. `/merchant/v1/*` |
| `method` | string | HTTP method filter: `ANY`, `GET`, `POST`, `PUT`, `DELETE` |
| `regex` | string | Full URL regex (overrides host/path matching) |

If `regex` is set, the glob fields are ignored. Use globs when you can, regex when you need capture groups or complex patterns.

### Target

Each rule has a `[rules.target]` section:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `host` | string | required | Target hostname |
| `port` | number | required | Target port |
| `scheme` | string | `http` | `http` or `https` |
| `path` | string | (preserved) | Override the path, or omit to keep the original |

### Full Example

```toml
[meta]
name = "dev"
description = "Dev environment redirects"
format_version = 1

[[rules]]
id = "web_app"
enabled = true
preserve_host = true

[rules.match]
host = "*.dev.myapp.com"
path = "/app/*"
not_path = "/app/api/*"

[rules.target]
host = "localhost"
port = 3000
scheme = "http"

[[rules]]
id = "api_server"
enabled = true
preserve_host = true

[rules.match]
host = "*.dev.myapp.com"
path = "/app/api/*"

[rules.target]
host = "localhost"
port = 9456
scheme = "http"

[[rules]]
id = "versioned_api"
enabled = false
preserve_host = false

[rules.match]
regex = "^https://api-(v[0-9]+)\\.dev\\.myapp\\.com/.*"

[rules.target]
host = "localhost"
port = 9000
scheme = "http"
```

## Menubar App

The optional menubar app (`giant-proxy-ui`) gives you a system tray icon with:

1. **Tray status** -- shows active profile and enabled rule names. Icon changes to indicate when the proxy is active.

2. **Tray menu** -- start/stop proxy, switch profiles, open dashboard. Starts the daemon automatically if needed.

3. **Dashboard window** -- full rule editor with create/edit/delete, profile management with import/export and drag-to-reorder, live traffic log with match/passthrough highlighting, settings panel, and About section.

Install it alongside the CLI:

```
brew install --cask bearded-giant/tap/giant-proxy-ui
```

Or build from source:

```
cd crates/giant-proxy-ui
pnpm install
pnpm tauri build
```

## Architecture

Giant Proxy is three binaries in a Cargo workspace:

```
crates/
  giantd/           daemon -- MITM proxy engine, control API, cert management
  giant-proxy/      CLI -- talks to daemon over Unix socket
  giant-proxy-ui/   Tauri menubar app -- same Unix socket API
```

The daemon (`giantd`) does the heavy lifting. It runs a hudsucker-based MITM proxy that intercepts HTTPS traffic, matches requests against loaded rules, and redirects to target hosts. It exposes a control API on a Unix socket at `~/.giant-proxy/giantd.sock`.

The CLI and menubar app are both thin clients that talk to the daemon's API. They never touch traffic directly.

### Key Paths

| Path | Purpose |
|------|---------|
| `~/.giant-proxy/config.toml` | Daemon configuration (ports, log level, routing mode) |
| `~/.giant-proxy/profiles/*.toml` | Rule profiles |
| `~/.giant-proxy/ca/` | Generated CA certificate and key |
| `~/.giant-proxy/giantd.sock` | Daemon Unix socket |
| `~/.giant-proxy/giantd.pid` | Daemon PID file |
| `~/.giant-proxy/state.json` | Runtime state |

## Auto-Start

To have the daemon start automatically on login:

```
giant-proxy daemon install
```

This installs a launchd agent on macOS or a systemd user unit on Linux. Remove it with `giant-proxy daemon uninstall`.

## Local Install (from source)

```
cargo install --path crates/giantd
cargo install --path crates/giant-proxy
giant-proxy init
```

This puts `giantd` and `giant-proxy` into `~/.cargo/bin/`. Then start it up:

```
giant-proxy init
giant-proxy on --profile preprod
```

To install the menubar app from source:

```
cd crates/giant-proxy-ui
pnpm install
pnpm tauri build
```

The `.app` bundle lands in `crates/giant-proxy-ui/src-tauri/target/release/bundle/macos/`. Drag it to Applications or run it directly.

## Development

```
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

Run the Tauri app in dev mode (hot-reload):

```
cd crates/giant-proxy-ui
pnpm install
pnpm tauri dev
```

## License

MIT
