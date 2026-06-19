use anyhow::Result;
use tracing::info;

use rverilog_frontend::parse_files;
use rverilog_hir::Design;
use rverilog_elab::elaborate;
use rverilog_sim::Interpreter;

pub fn run(args: &crate::cli::Args) -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_max_level(if args.verbose { tracing::Level::DEBUG } else { tracing::Level::INFO })
        .try_init();

    info!("Parsing files...");

    let defines = parse_defines(&args.define);

    match parse_files(&args.files, &args.include, &defines) {
        Ok(design) => {
            info!("Parsed {} modules", design.modules.len());
            print_modules(&design);

            info!("Elaborating top module: {}", args.top);
            let elaborated = elaborate(&design, &args.top, &[])?;

            info!("Creating interpreter");
            let mut interp = Interpreter::new(elaborated);

            if let Some(output) = &args.output {
                interp.set_vcd_output(output.clone());
            }

            info!("Running simulation");
            interp.run();

            Ok(())
        }
        Err(e) => {
            eprintln!("Parse error: {}", e);
            Err(anyhow::anyhow!("Failed to parse"))
        }
    }
}

fn parse_defines(defs: &[String]) -> Vec<(String, String)> {
    defs.iter().map(|s| {
        let parts: Vec<&str> = s.splitn(2, '=').collect();
        match parts.len() {
            1 => (parts[0].to_string(), String::new()),
            2 => (parts[0].to_string(), parts[1].to_string()),
            _ => unreachable!(),
        }
    }).collect()
}

fn print_modules(design: &Design) {
    for (name, _) in &design.modules {
        println!("Module: {}", name);
    }
}
