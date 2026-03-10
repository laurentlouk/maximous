mod db;
mod mcp;
mod tools;

use clap::Parser;
use std::sync::{Arc, Mutex};

#[derive(Parser)]
#[command(name = "maximous", about = "SQLite brain for multi-agent orchestration")]
struct Cli {
    /// Path to the SQLite database file
    #[arg(long, default_value = ".maximous/brain.db")]
    db: String,
}

fn main() {
    let cli = Cli::parse();
    let conn = db::open_db(&cli.db).expect("Failed to open database");
    eprintln!("maximous: database ready at {}", cli.db);

    let conn = Arc::new(Mutex::new(conn));
    mcp::run_stdio(conn);
}
