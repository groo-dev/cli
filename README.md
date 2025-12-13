# gr

A CLI tool for managing and running dev servers in monorepos.

## Installation

### Homebrew (macOS/Linux)

```bash
brew install groo-dev/tap/gr
```

### From Source

```bash
cargo install --git https://github.com/groo-dev/cli
```

## Usage

Run `gr` from anywhere inside your monorepo.

### Start dev servers

```bash
gr dev
```

Discovers all services with `dev` scripts and shows an interactive selector. Selected services run in parallel with color-coded output.

### View logs

```bash
gr logs           # Show last 10 lines from running services
gr logs -f        # Follow logs in real-time
gr logs -n 50     # Show last 50 lines
gr logs -n 50 -f  # Show last 50 lines, then follow
```

Tail logs from another terminal while `gr dev` is running. Supports viewing multiple services simultaneously with interleaved, color-coded output.

### Restart services

```bash
gr restart
```

Select running services to restart.

### Check status

```bash
gr status         # Status of services in current project
gr list           # List all projects with running services
```

### Stop services

```bash
gr stop           # Stop all services in current project
```

### Open in browser

```bash
gr open <service>
```

Opens the service URL in your default browser.

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
