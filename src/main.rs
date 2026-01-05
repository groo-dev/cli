mod auth;
mod commands;
mod config;
mod discovery;
mod pad;
mod project_config;
mod runner;
mod state;

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
    /// View logs for running services
    Logs {
        /// Number of lines to show per service
        #[arg(short = 'n', default_value = "10")]
        lines: usize,
        /// Follow log output
        #[arg(short = 'f', long)]
        follow: bool,
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
        Commands::Build { all } => commands::build::run(all).await,
        Commands::Restart => commands::restart::run().await,
        Commands::List => commands::list::run(),
        Commands::Status { project } => commands::status::run(project),
        Commands::Open { service } => commands::open::run(&service),
        Commands::Stop { project } => commands::stop::run(project),
        Commands::Logs { lines, follow } => commands::logs::run(lines, follow).await,
        Commands::Doctor => commands::doctor::run(),
        Commands::Auth { command } => match command {
            AuthCommands::Login { pat } => commands::auth::login::run(pat).await,
            AuthCommands::Status => commands::auth::status::run(),
            AuthCommands::Logout => commands::auth::logout::run(),
        },
        Commands::Pad { command } => match command {
            PadCommands::Add { text, files } => commands::pad::add::run(text, files).await,
        },
    }
}
