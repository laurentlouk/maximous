use clap::Parser;

#[derive(Parser)]
#[command(name = "maximous", about = "SQLite brain for multi-agent orchestration")]
struct Cli {
    /// Path to the SQLite database file
    #[arg(long, default_value = ".maximous/brain.db")]
    db: String,
}

fn main() {
    let cli = Cli::parse();
    println!("maximous starting with db: {}", cli.db);
}
