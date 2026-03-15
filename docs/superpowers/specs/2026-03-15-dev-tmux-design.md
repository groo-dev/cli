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

### Dependency check

On startup, check `which tmux`. If missing, print: `"tmux is required for groo dev. Install with: brew install tmux"` and exit.

Check tmux version >= 3.0 (for consistent `remain-on-exit` and status bar behavior).

### Creation

1. `groo dev` runs the existing service discovery and interactive selection flow (unchanged)
2. If a session named `groo-{project}` already exists, prompt: `"Session groo-{project} is already running. Attach to it? (Y/n)"`. On yes, attach. On no, ask `"Replace it? (y/N)"`. Only kill the old session after explicit confirmation.
3. Creates a new tmux session with all windows
4. Writes running services to `~/.groo/state.json` (see State Management)
5. Attaches current terminal to the session

### Nested tmux handling

If already inside a tmux session (`$TMUX` is set), use `tmux switch-client -t groo-{project}` instead of `tmux attach-session`. `attach-session` fails inside nested tmux.

### Exiting

- `Ctrl+C` in a service window kills that service only (window stays open due to `remain-on-exit`)
- `Ctrl+b d` (tmux detach) leaves everything running in background
- `groo stop` from another terminal can kill the session externally
- When the session is destroyed (all windows killed manually), state is stale until next `groo` command runs `clean_stale_pids()`

## Window Layout

```
Window 0: "all"     — interleaved logs from all services (default on attach)
Window 1: "api"     — raw output: cd accounts/api && npm run dev
Window 2: "web"     — raw output: cd accounts/web && npm run dev
Window 3: "pass"    — raw output: cd pass/api && npm run dev
```

### Window and service naming

The current `get_service_name()` in `discovery/services.rs` uses `:` as separator (e.g., `accounts:api`). Colons conflict with tmux's `session:window:pane` delimiter syntax and are unusual in filenames.

**Change**: Update `get_service_name()` to use `-` as separator instead of `:`. This produces names like `accounts-api` which work cleanly as both tmux window names and log filenames.

- Use short service name when unique (e.g., `api`, `web`)
- If names collide, use the full path-based name with `-` separator (e.g., `accounts-api`, `pass-api`)

### Navigation

- Standard tmux: `Ctrl+b 0-9` to jump, `Ctrl+b n/p` for next/prev
- Attach to window 0 ("all") by default

## Status Bar

```
[groo-gr] 0:all  1:api  2:web  3:pass                    3 services
```

- Left: session name + window list
- Right: service count
- All options set session-scoped (`tmux set-option -t groo-{project}`) to avoid polluting global tmux config
- Background: dark gray. Active window: bold bright text. Inactive: dimmed.
- No clock, no hostname — minimal.

## The "All" Window

Window 0 runs `groo dev --aggregate` (hidden internal subcommand).

### Behavior

- Tails all service log files simultaneously
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

### Log file resolution

The current log storage uses hash-based filenames: `get_service_log_file()` hashes the service path to produce `~/.groo/logs/{8-char-hash}.log`. The aggregate command needs a mapping from service name to log file.

**Approach**: Change log storage to use readable paths: `~/.groo/logs/{project}/{service-name}.log`. This makes the aggregate command straightforward and also makes log files human-browsable. Update `config.rs`:
- Add `get_project_logs_dir(project: &str) -> PathBuf` returning `~/.groo/logs/{project}/`
- Change `get_service_log_file()` to accept project name and service name, returning `~/.groo/logs/{project}/{service-name}.log`
- On first run with new scheme, old hash-based log files are simply ignored (they'll be overwritten naturally)

### Arguments

`groo dev --aggregate --project gr`

- `--aggregate`: hidden flag, triggers aggregate mode instead of normal dev flow
- `--project`: project name, used to find log directory `~/.groo/logs/{project}/`
- Service list derived by listing files in the project log directory (no need to pass explicitly)

### Shared code with `groo logs`

The existing `commands/logs.rs` already implements log file tailing with colored prefixes (`follow_logs`, `tail_log_file`). The aggregate window should reuse this tailing infrastructure. Extract the shared tailing logic into a common module (e.g., `log_tailer.rs`) used by both `groo logs` and the aggregate subprocess.

Note: The current `groo logs` code strips `[service]` prefixes from log lines before re-adding its own colored prefix. With the new `pipe-pane` log capture (which writes raw output without prefixes), this stripping logic should be removed. Log files will contain plain text lines with no service prefix — the prefix is added at display time by the reader.

### Exit

- Exits on `Ctrl+C`
- No timeout-based exit — runs until killed (services can be idle between requests)

## Per-Service Windows

- Each window runs: `sh -c "cd {path} && {dev_command}"` where `{dev_command}` comes from `service.dev_command` (not hardcoded to `npm run dev` — could be `make dev`, `wrangler dev`, etc.)
- No groo wrapper — command runs directly in tmux
- `remain-on-exit` enabled (session-scoped) so crash output stays visible
- Process gets its own process group naturally via tmux
- Log capture: each service window uses `tmux pipe-pane` to write output to the log file. The pipe command strips ANSI escape codes before writing, so log files contain clean text:
  ```
  tmux pipe-pane -t groo-{project}:{window} "sed 's/\x1b\[[0-9;]*[a-zA-Z]//g' >> ~/.groo/logs/{project}/{service-name}.log"
  ```
  The tmux window still shows raw colored output from the dev server. The log file gets clean text that the aggregate window and `groo logs` can read without garbled escape sequences.

## State Management

The current `state.json` tracks per-service PIDs. With tmux, groo doesn't spawn processes directly — tmux does.

**Approach**: After creating each tmux window, query the pane PID via `tmux display-message -t groo-{project}:{window} -p '#{pane_pid}'` and store it in state. This keeps the existing `ServiceState { pid, port }` model working. The `is_service_running()` function already prefers port-based checks over PID checks, so even if the PID is stale (e.g., tmux wrapper shell vs actual process), the port check remains reliable.

Additionally, store the tmux session name in state:

```rust
pub struct ProjectState {
    pub path: PathBuf,
    pub services: HashMap<String, ServiceState>,
    pub tmux_session: Option<String>,  // NEW
}
```

This allows `groo stop` and `groo restart` to detect and interact with tmux sessions.

## Commands That Need Updates

### `groo stop`

The current implementation uses port-based process killing, which still works with tmux. But it should also clean up the tmux session:

1. Run existing port-based stop logic (unchanged)
2. If `state.tmux_session` is set, kill the tmux session: `tmux kill-session -t {session}`
3. Clean up `~/.groo/state.json`

For selective stopping (user picks individual services), the port-based kill already works. Additionally, kill the tmux window: `tmux kill-window -t groo-{project}:{window}`.

### `groo restart`

Current implementation kills by port and respawns. With tmux:

1. Kill the service by port (unchanged)
2. Respawn in tmux: `tmux respawn-window -t groo-{project}:{window}` (re-runs the window's command)
3. Update PID in state

### `groo logs`

Keep unchanged. It works independently of the TUI — reads log files and tails them. Still useful when the tmux session is detached or for viewing logs of services started outside `groo dev`.

## Code Changes

### Removed

- Entire `dev_tui/` module: `mod.rs`, `app.rs`, `ui.rs`, `events.rs`, `logs.rs`, `stats.rs` (~1700 lines)
- Dependencies: `ratatui`, `crossterm`, `arboard`
- Dependencies: `sysinfo` (only used for TUI stats), `nix` (only used in `dev_tui/app.rs` for `killpg`)

### Kept as-is

- Service discovery (`discovery/`)
- Interactive service selection (dialoguer multiselect)
- Port detection (`discovery/ports.rs`)
- State management core (`state/tracker.rs`) — extended with `tmux_session` field
- All other commands (`build`, `list`, `status`, etc.)

### Modified

- `config.rs` — change log file path scheme from hash-based to `{project}/{service}.log`
- `discovery/services.rs` — change `get_service_name()` separator from `:` to `-`
- `commands/dev.rs` — call `dev_tmux::run()` instead of `dev_tui::run()`
- `commands/stop.rs` — add tmux session cleanup
- `commands/restart.rs` — use `tmux respawn-window` for tmux-managed services
- `commands/logs.rs` — remove `[service]` prefix stripping (log files no longer contain prefixes)
- `state/tracker.rs` — add `tmux_session` field to `ProjectState`
- `main.rs` — add hidden `--aggregate` flag to `Dev` subcommand (change from unit variant to struct variant)
- `runner/process.rs` — `spawn_service()` is still used by `groo restart` for non-tmux restarts; keep but fix the hardcoded `npm run dev` to use the actual `dev_command` parameter. `runner/output.rs` color functions are reused by aggregate.

### New

- `dev_tmux/mod.rs` — session creation, window management, status bar configuration
- `dev_tmux/aggregate.rs` — log aggregation subprocess (reuses tailing logic)
- `log_tailer.rs` — shared log tailing logic extracted from `commands/logs.rs`

### Estimated impact

Delete ~1700 lines, add ~400-500 lines. Net reduction of ~1200 lines.
