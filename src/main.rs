mod auth;
mod commands;
mod config;
mod dev_tmux;
mod discovery;
mod ops;
mod pad;
mod pass;
mod project_config;
mod runner;
mod state;
mod tasks;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "groo")]
#[command(about = "A CLI tool for managing and running dev servers in monorepos")]
#[command(version)]
struct Cli {
    /// Change to this directory before running
    #[arg(short = 'w', long = "workdir", global = true)]
    workdir: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start dev servers interactively
    Dev,
    /// Build services
    Build {
        /// Build all services without prompting
        #[arg(short, long)]
        all: bool,
        /// Build only services with changes
        #[arg(long)]
        changed: bool,
    },
    /// Lint services
    Lint {
        /// Lint all services without prompting
        #[arg(short, long)]
        all: bool,
        /// Lint only services with changes
        #[arg(long)]
        changed: bool,
    },
    /// Restart running services
    Restart,
    /// List all projects with running services
    List,
    /// Show status of services in a project
    Status {
        /// Project name (defaults to current directory)
        project: Option<String>,
    },
    /// Open a service in the browser
    Open {
        /// Service name to open
        service: String,
    },
    /// Stop all services in a project
    Stop {
        /// Project name (defaults to current directory)
        project: Option<String>,
    },
    /// Check project configuration for issues
    Doctor,
    /// Authenticate with Groo accounts
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },
    /// Add text or files to your Pad
    Pad {
        #[command(subcommand)]
        command: PadCommands,
    },
    /// Manage ops environment variables and secrets
    Ops {
        #[command(subcommand)]
        command: OpsCommands,
    },
    /// Password manager
    Pass {
        #[command(subcommand)]
        command: Option<PassCommands>,
    },
    /// Task tracking
    Tasks {
        #[command(subcommand)]
        command: TasksCommands,
    },
}

#[derive(Subcommand)]
enum AuthCommands {
    /// Login to Groo
    Login {
        /// Use Personal Access Token instead of browser OAuth
        #[arg(long)]
        pat: bool,
    },
    /// Show current authentication status
    Status,
    /// Logout and clear credentials
    Logout,
}

#[derive(Subcommand)]
enum PadCommands {
    /// Add text or files to your pad list
    Add {
        /// Text to add (reads from stdin if not provided)
        text: Option<String>,
        /// File(s) to upload (supports globs and folders)
        #[arg(short = 'f', long = "file", action = clap::ArgAction::Append)]
        files: Vec<PathBuf>,
    },
    /// View and manage your pad items
    List,
}

#[derive(Subcommand)]
enum OpsCommands {
    /// Link service to ops application
    Link {
        /// Service name (interactive picker if not provided)
        #[arg(short, long)]
        service: Option<String>,
    },
    /// Unlink service from ops
    Unlink {
        /// Service name (interactive picker if not provided)
        #[arg(short, long)]
        service: Option<String>,
    },
    /// Manage environment variables and secrets
    Env {
        #[command(subcommand)]
        command: OpsEnvCommands,
    },
}

#[derive(Subcommand)]
enum OpsEnvCommands {
    /// List all env vars and secrets
    List {
        /// Service name (interactive picker if not provided)
        #[arg(short, long)]
        service: Option<String>,
        /// Environment (development, staging, production)
        #[arg(long, default_value = "development")]
        env: String,
    },
    /// Show diff between local and remote
    Diff {
        /// Service name (interactive picker if not provided)
        #[arg(short, long)]
        service: Option<String>,
        /// Environment (development, staging, production)
        #[arg(long, default_value = "development")]
        env: String,
    },
    /// Pull remote config to local env file
    Pull {
        /// Service name (interactive picker if not provided)
        #[arg(short, long)]
        service: Option<String>,
        /// Environment (development, staging, production)
        #[arg(long, default_value = "development")]
        env: String,
    },
    /// Push local env file to remote
    Push {
        /// Service name (interactive picker if not provided)
        #[arg(short, long)]
        service: Option<String>,
        /// Environment (development, staging, production)
        #[arg(long, default_value = "development")]
        env: String,
    },
}

#[derive(Subcommand)]
enum PassCommands {
    /// Search and copy a password
    Get {
        /// Search query (name, username, or URL)
        query: String,
        /// Copy username instead of password
        #[arg(short, long)]
        username: bool,
        /// Copy TOTP code
        #[arg(short, long)]
        totp: bool,
        /// Print to stdout instead of clipboard
        #[arg(short, long)]
        show: bool,
    },
    /// Add a new password item
    Add,
    /// Migrate secrets from Keychain to Pass
    Migrate,
    /// Generate a password
    Generate {
        /// Password length
        #[arg(short, long, default_value = "20")]
        length: usize,
        /// Exclude uppercase letters
        #[arg(long)]
        no_uppercase: bool,
        /// Exclude lowercase letters
        #[arg(long)]
        no_lowercase: bool,
        /// Exclude numbers
        #[arg(long)]
        no_numbers: bool,
        /// Exclude symbols
        #[arg(long)]
        no_symbols: bool,
        /// Generate passphrase instead
        #[arg(long)]
        passphrase: bool,
        /// Word count for passphrase
        #[arg(long, default_value = "4")]
        words: usize,
        /// Separator for passphrase
        #[arg(long, default_value = "-")]
        separator: String,
        /// Print instead of copy to clipboard
        #[arg(short, long)]
        print: bool,
    },
}

#[derive(Subcommand)]
enum TasksCommands {
    /// Add a new task
    Add {
        /// Task title
        title: String,
        /// Project name (auto-detected from directory if not specified)
        #[arg(short, long)]
        project: Option<String>,
        /// Priority: low, medium, high
        #[arg(long)]
        priority: Option<String>,
        /// Tags (comma-separated)
        #[arg(long, value_delimiter = ',')]
        tags: Option<Vec<String>>,
        /// Task description
        #[arg(short, long)]
        description: Option<String>,
    },
    /// List tasks
    List {
        /// Filter by project (auto-detected from directory if not specified)
        #[arg(short, long)]
        project: Option<String>,
        /// Filter by status: backlog, open, in_progress, done, archived
        #[arg(short, long)]
        status: Option<String>,
        /// Include archived tasks
        #[arg(short, long)]
        all: bool,
    },
    /// Search tasks
    Search {
        /// Search query
        query: String,
    },
    /// Show task details
    Show {
        /// Task ID
        id: String,
    },
    /// Start a task (set to in_progress)
    Start {
        /// Task ID
        id: String,
    },
    /// Complete a task (set to done)
    Done {
        /// Task ID
        id: String,
    },
    /// Archive a task
    Archive {
        /// Task ID
        id: String,
    },
    /// Add a comment to a task
    Comment {
        /// Task ID
        id: String,
        /// Comment content
        content: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Change working directory if specified
    if let Some(workdir) = &cli.workdir {
        std::env::set_current_dir(workdir)
            .with_context(|| format!("Failed to change directory to: {}", workdir.display()))?;
    }

    match cli.command {
        Commands::Dev => commands::dev::run().await,
        Commands::Build { all, changed } => commands::build::run(all, changed).await,
        Commands::Lint { all, changed } => commands::lint::run(all, changed).await,
        Commands::Restart => commands::restart::run().await,
        Commands::List => commands::list::run(),
        Commands::Status { project } => commands::status::run(project),
        Commands::Open { service } => commands::open::run(&service),
        Commands::Stop { project } => commands::stop::run(project),
        Commands::Doctor => commands::doctor::run().await,
        Commands::Auth { command } => match command {
            AuthCommands::Login { pat } => commands::auth::login::run(pat).await,
            AuthCommands::Status => commands::auth::status::run(),
            AuthCommands::Logout => commands::auth::logout::run(),
        },
        Commands::Pad { command } => match command {
            PadCommands::Add { text, files } => commands::pad::add::run(text, files).await,
            PadCommands::List => commands::pad::list::run().await,
        },
        Commands::Ops { command } => match command {
            OpsCommands::Link { service } => commands::ops::link::run_link(service).await,
            OpsCommands::Unlink { service } => commands::ops::link::run_unlink(service).await,
            OpsCommands::Env { command } => match command {
                OpsEnvCommands::List { service, env } => {
                    commands::ops::env::run_list(service, env).await
                }
                OpsEnvCommands::Diff { service, env } => {
                    commands::ops::env::run_diff(service, env).await
                }
                OpsEnvCommands::Pull { service, env } => {
                    commands::ops::env::run_pull(service, env).await
                }
                OpsEnvCommands::Push { service, env } => {
                    commands::ops::env::run_push(service, env).await
                }
            },
        },
        Commands::Pass { command } => match command {
            None => commands::pass::list::run().await,
            Some(PassCommands::Get {
                query,
                username,
                totp,
                show,
            }) => commands::pass::get::run(&query, username, totp, show).await,
            Some(PassCommands::Add) => commands::pass::add::run().await,
            Some(PassCommands::Migrate) => commands::pass::migrate::run().await,
            Some(PassCommands::Generate {
                length,
                no_uppercase,
                no_lowercase,
                no_numbers,
                no_symbols,
                passphrase,
                words,
                separator,
                print,
            }) => {
                commands::pass::generate::run(
                    length,
                    no_uppercase,
                    no_lowercase,
                    no_numbers,
                    no_symbols,
                    passphrase,
                    words,
                    &separator,
                    print,
                )
                .await
            }
        },
        Commands::Tasks { command } => match command {
            TasksCommands::Add {
                title,
                project,
                priority,
                tags,
                description,
            } => commands::tasks::add::run(title, project, priority, tags, description).await,
            TasksCommands::List {
                project,
                status,
                all,
            } => commands::tasks::list::run(project, status, all).await,
            TasksCommands::Search { query } => commands::tasks::search::run(query).await,
            TasksCommands::Show { id } => commands::tasks::show::run(id).await,
            TasksCommands::Start { id } => commands::tasks::start::run(id).await,
            TasksCommands::Done { id } => commands::tasks::done::run(id).await,
            TasksCommands::Archive { id } => commands::tasks::archive::run(id).await,
            TasksCommands::Comment { id, content } => {
                commands::tasks::comment::run(id, content).await
            }
        },
    }
}
