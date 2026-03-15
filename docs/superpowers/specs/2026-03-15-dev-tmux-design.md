# Design: Replace dev TUI with tmux-based session

## Problem

The current `groo dev` TUI (ratatui + crossterm) has persistent usability issues:

- Mouse capture is enabled but never handled — native text selection is impossible
- Scrolling uses hardcoded heights instead of actual terminal dimensions
- Copying requires vim-style visual mode (`v` → `j/k` → `y`) instead of native mouse selection
- The sidebar permanently consumes 25 columns from log space
- Custom UI code (~1700 lines) is bug-prone and difficult to maintain

## Solution

Replace the custom TUI with tmux session orchestration. `groo dev` creates a tmux session with one window per service plus an "all" window for interleaved logs. All output is plain terminal text — native selection, copying, and scrolling work out of the box.

## Session Lifecycle

### Creation

1. `groo dev` runs the existing service discovery and interactive selection flow (unchanged)
2. Checks for tmux: `which tmux`. If missing, prints: `"tmux is required for groo dev. Install with: brew install tmux"`
3. If a session named `groo-{project}` already exists, kills it
4. Creates a new tmux session with all windows
5. Writes running services to `~/.groo/state.json` (unchanged)
6. Attaches current terminal to the session

### Exiting

- `Ctrl+C` in a service window kills that service only
- `Ctrl+b d` (tmux detach) leaves everything running in background
- `groo stop` from another terminal kills the session externally
- When the session is destroyed, clean up `~/.groo/state.json`

## Window Layout

```
Window 0: "all"     — interleaved logs from all services (default on attach)
Window 1: "api"     — raw output: cd accounts/api && npm run dev
Window 2: "web"     — raw output: cd accounts/web && npm run dev
Window 3: "pass"    — raw output: cd pass/api && npm run dev
```

### Window naming

- Use short service name (e.g., `api`, `web`)
- If names collide, use path-based name for disambiguation (e.g., `api:accounts`, `api:pass`)

### Navigation

- Standard tmux: `Ctrl+b 0-9` to jump, `Ctrl+b n/p` for next/prev
- Attach to window 0 ("all") by default

## Status Bar

```
[groo-gr] 0:all  1:api  2:web  3:pass                    3 services
```

- Left: session name + window list
- Right: service count
- Styled via `tmux set-option` at session creation time
- Background: dark gray. Active window: bold bright text. Inactive: dimmed.
- No clock, no hostname — minimal.

## The "All" Window

Window 0 runs `groo dev --aggregate` (hidden internal subcommand).

### Behavior

- Tails all service log files simultaneously from `~/.groo/logs/{project}/{service}.log`
- Interleaves by timestamp
- Prints plain stdout with colored `[service-name]` prefixes:

```
[api]  Listening on port 3001
[web]  Network: http://localhost:5173
[api]  GET /health 200 3ms
[pass] Ready on port 5000
```

- Each service gets a consistent color (same 8-color rotation as current TUI)
- No raw mode, no cursor manipulation — just `println!`
- Native terminal selection, copying, scrolling all work

### Arguments

`groo dev --aggregate --services api,web,pass --project gr`

- `--services`: comma-separated service names to tail
- `--project`: project name for log directory lookup

### Exit

- Exits on `Ctrl+C`
- Exits when all log files stop being written to (all services dead)

## Per-Service Windows

- Each window runs: `sh -c "cd {path} && {dev_command}"`
- No groo wrapper — command runs directly in tmux
- `remain-on-exit` enabled so crash output stays visible
- Process gets its own process group naturally via tmux

## Process Management & Cleanup

### `groo stop`

1. Kills tmux session: `tmux kill-session -t groo-gr`
2. tmux sends SIGHUP to all processes in all windows
3. Clean up `~/.groo/state.json`

### State tracking

- Write to `~/.groo/state.json` before creating session (unchanged)
- `groo list`, `groo status`, `groo stop` continue to work as before

## Code Changes

### Removed

- Entire `dev_tui/` module: `mod.rs`, `app.rs`, `ui.rs`, `events.rs`, `logs.rs`, `stats.rs` (~1700 lines)
- Dependencies: `ratatui`, `crossterm`, `arboard`

### Kept as-is

- Service discovery (`discovery/`)
- Interactive service selection (dialoguer multiselect)
- Port detection (`discovery/ports.rs`)
- Log file writing (`~/.groo/logs/`)
- State management (`~/.groo/state.json`)
- All other commands (`build`, `stop`, `list`, `status`, etc.)

### New

- `dev_tmux/mod.rs` — session creation, window management, status bar configuration
- `dev_tmux/aggregate.rs` — log file tailer for the "all" window
- `--aggregate` hidden subcommand added to `main.rs`

### Estimated impact

Delete ~1700 lines, add ~300-400 lines. Net reduction of ~1300 lines.
