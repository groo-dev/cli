# groo

A CLI tool for managing and running dev servers in monorepos.

## Installation

### Homebrew (macOS/Linux)

```bash
brew install groo-dev/tap/groo
```

### From Source

```bash
cargo install --git https://github.com/groo-dev/cli
```

## Usage

Run `groo` from anywhere inside your monorepo.

### Sign in

```bash
groo auth login
```

The CLI opens `me.groo.dev` and signs in with OAuth authorization code + PKCE.
No personal access token or client secret is required. Access and refresh tokens
are stored in the operating system credential store.

For SSH sessions, containers, or other environments without a local browser:

```bash
groo auth login --device
```

Use `groo auth status` to inspect the current session and `groo auth logout` to
revoke it and remove local credentials. A working operating-system credential
store is required; credential-storage errors stop the command and are reported.

### Start dev servers

```bash
groo dev
```

Discovers all services with `dev` scripts and shows an interactive selector. Selected services run in parallel with color-coded output.

### View logs

```bash
groo logs           # Show last 10 lines from running services
groo logs -f        # Follow logs in real-time
groo logs -n 50     # Show last 50 lines
groo logs -n 50 -f  # Show last 50 lines, then follow
```

Tail logs from another terminal while `groo dev` is running. Supports viewing multiple services simultaneously with interleaved, color-coded output.

### Restart services

```bash
groo restart
```

Select running services to restart.

### Check status

```bash
groo status         # Status of services in current project
groo list           # List all projects with running services
```

### Stop services

```bash
groo stop           # Stop all services in current project
```

### Open in browser

```bash
groo open <service>
```

Opens the service URL in your default browser.

### Scaffold a new project

```bash
groo launchpad --config launchpad.json
```

Scaffolds a complete multi-project repo from a JSON config — creates projects, installs dependencies, generates config files, sets up Cloudflare resources, writes project docs, and initializes git. Designed to be driven by the [launchpad skill](https://github.com/groo-dev/skills) which collects requirements conversationally and produces the config.

```bash
groo launchpad --config launchpad.json --clean  # Delete previous failed run and start fresh
```

## Port Detection

Ports are detected automatically based on framework:

| Framework | Detection | Default |
|-----------|-----------|---------|
| Next.js | `-p`/`--port` flag in dev script | 3000 |
| Vite | `port` in vite.config.* | 5173 |
| Wrangler | `port` in wrangler.jsonc/toml | 8787 |
| Other | `-p`/`--port` flag in dev script | — |

## Global Options

```
-w, --workdir <PATH>  Run from a different directory
-h, --help            Print help
-V, --version         Print version
```

## License

MIT
