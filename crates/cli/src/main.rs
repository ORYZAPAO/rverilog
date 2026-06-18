mod cli;
mod driver;

use anyhow::Result;
use cli::Args;
use clap::Parser;

fn main() -> Result<()> {
    let args = Args::parse();
    driver::run(&args)
}
