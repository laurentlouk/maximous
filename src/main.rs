mod db;
mod mcp;
mod tools;
mod web;

use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Parser)]
#[command(name = "maximous", about = "SQLite brain for multi-agent orchestration")]
struct Cli {
    /// Path to the SQLite database file
    #[arg(long, default_value = ".maximous/brain.db", global = true)]
    db: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the web dashboard
    Dashboard {
        /// Web dashboard port
        #[arg(long, default_value = "8375")]
        port: u16,
    },
}

/// Resolve the database path relative to the project root.
/// If the path is absolute, use as-is.
/// If relative, find the project root (nearest ancestor with .git/) and resolve from there.
/// Falls back to cwd if no .git/ is found.
fn resolve_db_path(db: &str) -> String {
    let path = Path::new(db);
    if path.is_absolute() {
        return db.to_string();
    }

    let project_root = find_project_root().unwrap_or_else(|| {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    });

    project_root.join(db).to_string_lossy().to_string()
}

/// Walk up from cwd to find the nearest directory containing .git/
fn find_project_root() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if dir.join(".git").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let db_path = resolve_db_path(&cli.db);
    let conn = db::open_db(&db_path).expect("Failed to open database");
    eprintln!("maximous: database ready at {}", db_path);

    let conn = Arc::new(Mutex::new(conn));

    match cli.command {
        Some(Commands::Dashboard { port }) => {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let url = format!("http://127.0.0.1:{}", port);
                // Spawn browser open in background so it doesn't block the server
                let url_clone = url.clone();
                tokio::spawn(async move {
                    // Small delay to let the server start binding
                    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                    let _ = open_browser(&url_clone);
                });
                web::serve(conn, port).await;
            });
        }
        None => {
            mcp::run_stdio(conn);
        }
    }
}

fn open_browser(url: &str) -> std::io::Result<std::process::Child> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "unsupported platform for browser open",
        ))
    }
}
