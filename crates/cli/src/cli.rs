use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "rverilog")]
#[command(about = "Rust Verilog-2001 subset simulator")]
pub struct Args {
    /// Top module name
    #[arg(short, long)]
    pub top: String,

    /// Include directories
    #[arg(short, long, num_args = 1..)]
    pub include: Vec<PathBuf>,

    /// Macro definitions
    #[arg(short, long, num_args = 1..)]
    pub define: Vec<String>,

    /// Output VCD file
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Maximum simulation time
    #[arg(long)]
    pub max_time: Option<u64>,

    /// Verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Input Verilog files
    #[arg(required = true)]
    pub files: Vec<PathBuf>,
}
