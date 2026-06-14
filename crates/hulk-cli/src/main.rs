use std::env;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: hulkc run <file>");
        std::process::exit(1);
    }
    let cmd = &args[1];
    match cmd.as_str() {
        "run" => {
            let path = &args[2];
            let src = fs::read_to_string(path)?;
            let prog = hulk_parser::parse(&src).map_err(|e| format!("parse error: {}", e))?;
            hulk_checker::check(&prog).map_err(|e| format!("type error: {}", e))?;
            // For now, we don't generate bytecode; just exit.
            println!("Parsed and type-checked successfully (stub).");
        }
        _ => {
            eprintln!("Unknown command: {}", cmd);
            std::process::exit(1);
        }
    }
    Ok(())
}
