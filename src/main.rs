mod db;
mod mcp;
mod tools;
mod web;

use clap::Parser;
use std::sync::{Arc, Mutex};

#[derive(Parser)]
#[command(name = "maximous", about = "SQLite brain for multi-agent orchestration")]
struct Cli {
    /// Path to the SQLite database file
    #[arg(long, default_value = ".maximous/brain.db")]
    db: String,

    /// Enable web dashboard
    #[arg(long)]
    web: bool,

    /// Web dashboard port
    #[arg(long, default_value = "8375")]
    port: u16,
}

fn main() {
    let cli = Cli::parse();
    let conn = db::open_db(&cli.db).expect("Failed to open database");
    eprintln!("maximous: database ready at {}", cli.db);

    let conn = Arc::new(Mutex::new(conn));

    if cli.web {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(web::serve(conn, cli.port));
    } else {
        mcp::run_stdio(conn);
    }
}
