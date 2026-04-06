# Launchpad CLI Command — Design Spec

**Date:** 2026-04-06
**Status:** Approved

## Overview

Add a `groo launchpad` command to the groo CLI that handles all deterministic scaffolding work for new projects. The LLM-powered launchpad skill collects requirements conversationally (Phase 1), writes a config JSON, and hands off to the CLI. The CLI executes everything — scaffolding, dependency installation, config file generation, resource creation, project files, and git init.

**Goal:** Save time for users and tokens for the LLM by moving predictable work out of the AI and into a deterministic CLI pipeline.

## Command Interface

```
groo launchpad --config <path>
groo launchpad --config <path> --clean
```

- `--config <path>` — path to the JSON config file written by the LLM
- `--clean` — delete everything from a previous failed run (tracked in state file), then start fresh

## Config Schema

```json
{
  "name": "myapp",
  "root": ".",
  "description": "A task management app with team collaboration",
  "domain": "myapp.groo.bot",
  "projects": [
    {
      "name": "dashboard",
      "type": "web",
      "auth": "clerk"
    },
    {
      "name": "api",
      "type": "api-worker",
      "auth": "clerk",
      "email": "resend",
      "resources": ["d1", "kv"]
    },
    {
      "name": "email-handler",
      "type": "lightweight-worker",
      "resources": []
    }
  ],
  "create_resources": true,
  "remote": false
}
```

### Fields

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | Project name, used as Cloudflare resource prefix |
| `root` | string | yes | `"."` for current directory, or a directory name to create |
| `description` | string | yes | One-line project description |
| `domain` | string | no | Application domain (required if any api-worker exists) |
| `projects` | array | yes | At least one project |
| `create_resources` | bool | yes | Whether to create Cloudflare resources via wrangler |
| `remote` | bool | yes | Whether to add `remote: true` to resource bindings |

### Project Fields

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | Project directory name |
| `type` | enum | yes | `web`, `api-worker`, `lightweight-worker`, `ios`, `android` |
| `auth` | enum | no | `clerk`, `better-auth`, `simple` |
| `email` | enum | no | `resend` |
| `resources` | array | no | `d1`, `r2`, `kv`, `queues`, `ai-gateway` |

## Validation

Validation is implemented in Rust (`validation.rs`) with clear, actionable error messages. All errors are reported at once so the LLM can fix them in a single pass.

### Structural Validation

Handled automatically by serde deserialization — missing required fields, invalid enum values, wrong types. Serde errors are already clear enough for the LLM.

### Business Logic Validation

Each rule produces a message that names the field, explains what's wrong, and suggests the fix.

**Rules:**

- `name` must be non-empty and a valid directory name (alphanumeric, hyphens, underscores)
- `root` must be `"."` (current dir is empty or has no conflicting subdirs) or a name that doesn't already exist
- `projects` must have at least one entry
- Project names must be unique
- Project names must be valid directory names
- `domain` is required if any project has type `api-worker`
- `web` projects must not have `resources`
- `web` projects must not have `email`
- `lightweight-worker` projects must not have `auth` or `email`
- `ios` and `android` projects must not have `resources`, `auth`, or `email`

### Error Output Format

```
  Launchpad 🚀

  ✗ Config validation failed (2 errors):

  1. Project 'dashboard': web projects don't have Cloudflare resource
     bindings. Remove 'resources' or change type to 'api-worker'.

  2. Missing 'domain': required when any project is an API worker.
     Add a domain like "myapp.groo.bot".
```

## Execution Pipeline

After validation, the CLI runs steps sequentially. Each step succeeds or errors — no fallbacks, no skipping, no "try this then try that." If it works once, it works always.

### Steps

```
1.  Validate config
2.  Create root directory (if root != ".")
3.  For each project (sequential):
    a. Scaffold (npm create vite / npm create cloudflare with non-interactive flags)
    b. Install dependencies (npm install with specific package lists)
    c. Rename directory if needed
4.  Generate ports (one random 5-digit port per project, guaranteed unique)
5.  Write config files (wrangler.jsonc, vite.config.ts, drizzle.config.ts, tsconfig)
6.  Write package.json scripts
7.  Write boilerplate code (entry points, api client, config.ts)
8.  Write env example files (.env.example, .dev.vars.example)
9.  Create Cloudflare resources via wrangler (if create_resources is true)
10. Bind resource IDs to wrangler.jsonc
11. Run cf-typegen per worker
12. Run db:generate + db:migrate:local (for workers with D1)
13. Write project files (CLAUDE.md, README.md, TODO.md, .gitignore, GitHub Actions, .claude/settings.local.json)
14. Git init + initial commit
```

### Scaffolding Commands

- **Web projects:** `npm create vite@latest {name} -- --template react-ts`
- **Cloudflare Workers:** `npm create cloudflare@latest {name}` with flags to skip interactive prompts (exact flags TBD during implementation — verify non-interactive mode works reliably)
- **iOS/Android:** Print instruction asking user to create via Xcode/Android Studio, then continue

### Dependency Installation

**Web projects:**
```bash
npm install @tanstack/react-router @tanstack/react-query axios date-fns clsx tailwind-merge class-variance-authority lucide-react
npm install -D tailwindcss @tailwindcss/vite typescript @vitejs/plugin-react eslint wrangler
```

Plus auth SDK if selected:
- Clerk: `npm install @clerk/clerk-react @clerk/themes`
- Better Auth: `npm install better-auth`

**API workers:**
```bash
npm install hono drizzle-orm
npm install -D drizzle-kit wrangler @types/node
```

Plus backend SDKs:
- Clerk: `npm install @clerk/backend`
- Resend: `npm install resend`

**Lightweight workers:** Only install what's specifically needed. No Hono or Drizzle.

### On Failure

- Execution stops immediately
- The error message includes the exact command that failed and its output
- A summary shows what succeeded and what wasn't started
- No automatic cleanup — files created by completed steps remain on disk
- State is saved to `.launchpad-state.json` for resume

## State & Resume

### State File

The CLI writes `.launchpad-state.json` in the root directory as it progresses:

```json
{
  "config_hash": "abc123",
  "completed_steps": [
    { "step": "scaffold", "project": "dashboard", "result": "ok" },
    { "step": "install_deps", "project": "dashboard", "result": "ok" },
    { "step": "scaffold", "project": "api", "result": "failed", "error": "npm ERR! ..." }
  ],
  "created_resources": [
    { "type": "d1", "name": "myapp-d1", "id": "abc-123" }
  ]
}
```

### Resume Behavior

Re-running `groo launchpad --config launchpad.json` when a state file exists:

- Detects `.launchpad-state.json`
- Skips completed steps
- Resumes from the failure point
- If config hash changed (LLM fixed the config), all steps from the first failed step onward are re-run. Completed steps before the failure point are still skipped — they produced correct output from the original config and don't need to be redone

```
  Launchpad 🚀

  Resuming from previous run...

  Creating web app "dashboard"
  ✓ Already complete — skipped

  Creating API worker "api"
  ⠋ Scaffolding with Cloudflare Worker...
```

### Clean Start

`groo launchpad --config launchpad.json --clean` deletes everything tracked in the state file — directories, generated files, Cloudflare resources — then runs fresh:

```
  Launchpad 🚀

  Cleaning previous run...
  ✓ Removed dashboard/
  ✓ Removed api/
  ✓ Deleted D1 database "myapp-d1"

  Starting fresh...
```

The state file is deleted on successful completion. It only exists during in-progress or failed runs.

## Terminal UI

Uses `crossterm` (already in the CLI) for terminal control and `console` (already in the CLI) for colors.

### Three States Per Step

**Running** — spinner animates, command output streams below:
```
  Creating web app "dashboard"
  ⠋ Scaffolding with Vite + React + TypeScript
    > Scaffolding project in ./dashboard...
    > Done. Now run:
```

Max ~5 most recent output lines shown. Older lines scroll off.

**Succeeded** — streaming output collapses, replaced with checkmark:
```
  Creating web app "dashboard"
  ✓ Scaffolded with Vite + React + TypeScript
```

**Failed** — output stays visible, execution stops:
```
  Creating web app "dashboard"
  ✗ Scaffolding failed
    > npm ERR! could not determine executable to run
```

### Overall Layout

```
  Launchpad 🚀

  Creating web app "dashboard"
  ✓ Scaffolded with Vite + React + TypeScript
  ✓ Installed 14 packages

  Creating API worker "api"
  ✓ Scaffolded with Cloudflare Worker
  ✓ Installed 4 packages (hono, drizzle-orm...)
  ✓ wrangler.jsonc — D1, KV bindings configured
  ✓ Hono entry point with /v1 base path

  Setting up Cloudflare resources
  ✓ Created D1 database "myapp-d1"
  ✓ Created KV namespace "myapp-kv"
  ✓ Bound resource IDs to wrangler.jsonc

  Writing project files
  ✓ CLAUDE.md
  ✓ README.md
  ✓ TODO.md
  ✓ .github/workflows/deploy-api.yml
  ✓ .github/workflows/deploy-dashboard.yml
  ✓ .gitignore

  Initializing git
  ✓ Initial commit — ready to go

  Done! Run "groo dev" to start building.
```

### Implementation

- Track cursor position for each active step
- On command completion: move cursor up, clear streaming lines, replace spinner with ✓/✗
- `crossterm::terminal` for raw mode, cursor movement, line clearing
- `console` crate for colors (green ✓, red ✗, dim log lines)
- Spinner uses braille pattern characters (⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏)

## Template Engine

**Engine:** Tera (Jinja2-inspired, Rust crate)

Templates are embedded at compile time via `include_str!` for single-binary distribution.

### Template List

| Template | Used for |
|---|---|
| `wrangler.jsonc.tera` | All workers |
| `vite.config.ts.tera` | Web projects |
| `drizzle.config.ts.tera` | Workers with D1 |
| `hono-entry.ts.tera` | API worker entry point |
| `axios-client.ts.tera` | Web project API client |
| `config-worker.ts.tera` | Worker config.ts |
| `config-web.ts.tera` | Web config.ts |
| `schema.ts.tera` | Drizzle starter schema |
| `env.example.tera` | Web .env.example |
| `dev.vars.example.tera` | Worker .dev.vars.example |
| `deploy-worker.yml.tera` | Worker GitHub Action workflow |
| `deploy-web.yml.tera` | Web GitHub Action workflow |
| `gitignore.tera` | Root .gitignore |
| `claude.md.tera` | CLAUDE.md |
| `readme.md.tera` | README.md |
| `todo.md.tera` | TODO.md |
| `settings.local.json.tera` | .claude/settings.local.json |

### Template Context

A Rust struct serialized to a Tera `Context` containing all derived values:

```rust
struct TemplateContext {
    name: String,
    prefix: String,
    description: String,
    domain: Option<String>,
    zone: Option<String>,        // root domain derived from domain
    projects: Vec<ProjectContext>,
    today: String,               // compatibility_date
    has_api_worker: bool,
    has_d1: bool,
    // ... other derived flags
}

struct ProjectContext {
    name: String,
    project_type: String,
    port: u16,
    auth: Option<String>,
    email: Option<String>,
    resources: Vec<String>,
    has_d1: bool,
    has_r2: bool,
    // ... resource flags for conditionals
}
```

## CLI Architecture

### File Structure

```
src/commands/launchpad/
├── mod.rs              # Command entry point, orchestrator
├── config.rs           # Config struct, serde deserialization
├── validation.rs       # Business rule validation
├── pipeline.rs         # Step execution pipeline
├── state.rs            # .launchpad-state.json read/write
├── scaffold.rs         # npm create wrappers
├── deps.rs             # npm install per project type
├── templates.rs        # Tera context building + rendering
└── ui.rs               # Terminal UI (spinner, streaming, collapse)

templates/launchpad/
├── wrangler.jsonc.tera
├── vite.config.ts.tera
├── drizzle.config.ts.tera
├── hono-entry.ts.tera
├── axios-client.ts.tera
├── config-worker.ts.tera
├── config-web.ts.tera
├── schema.ts.tera
├── env.example.tera
├── dev.vars.example.tera
├── deploy-worker.yml.tera
├── deploy-web.yml.tera
├── gitignore.tera
├── claude.md.tera
├── readme.md.tera
├── todo.md.tera
└── settings.local.json.tera
```

### Dependencies Added to Cargo.toml

- `tera` — template engine

All other dependencies (`crossterm`, `console`, `serde`, `serde_json`, `tokio`, `anyhow`) are already in the project.

## Skill Integration

### Workflow

1. LLM runs Phase 1 conversation (unchanged — same questions)
2. LLM builds config JSON from answers
3. LLM shows config to user for confirmation
4. LLM writes `launchpad.json` to target directory
5. LLM runs `groo launchpad --config launchpad.json`
6. On success → LLM tells user to run `groo dev`
7. On failure → LLM reads error, fixes config or advises user, retries

### SKILL.md Changes Summary

The following changes should be made to `SKILL.md` in a separate session:

**Keep unchanged:**
- Phase 1 (Gather Requirements) — all 9 questions stay the same

**Replace Phases 2, 3, 4 with:**
- A new Phase 2 that:
  1. Builds the config JSON from Phase 1 answers
  2. Shows the JSON to the user for confirmation
  3. Writes `launchpad.json` to the target directory
  4. Runs `groo launchpad --config launchpad.json`
  5. On success, instructs user to run `groo dev`
  6. On failure, reads error output, fixes config if possible, retries

**Add:**
- Config JSON reference section documenting the full schema (fields, types, valid enum values, required vs optional) so the LLM knows exactly what shape to produce
- Error handling guidance — how to interpret CLI error output and fix common issues

**Remove:**
- All file templates (wrangler.jsonc, vite.config.ts, etc.) — CLI owns these now
- All shell commands for scaffolding, installing deps, creating resources — CLI handles these
- All boilerplate code examples (Hono entry point, axios client, config.ts patterns)
- GitHub Actions workflow templates
- Project file templates (CLAUDE.md, README.md, TODO.md, .gitignore, settings.local.json)
- The "Important Rules" about file generation — these become CLI implementation details

**Result:** SKILL.md goes from ~669 lines to ~150 lines. The skill focuses purely on the conversation; the CLI owns all execution.
