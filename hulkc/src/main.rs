use std::fs;
use std::io::{self, BufRead, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process;

use hulk_interpreter::Interpreter;
use hulk_parser::{compile, parse_program, PipelineError};
use hulk_semantic::check;

// ── Interface-compliant error reporting ───────────────────────────────────────
// Format: (line,col) TYPE: message  →  stderr
// Exit codes: 1=LEXICAL  2=SYNTACTIC  3=SEMANTIC

fn emit_lexical(line: u32, col: u32, msg: &str) {
    eprintln!("({line},{col}) LEXICAL: {msg}");
}

fn emit_syntactic(line: u32, col: u32, msg: &str) {
    eprintln!("({line},{col}) SYNTACTIC: {msg}");
}

fn emit_semantic(line: u32, col: u32, msg: &str) {
    eprintln!("({line},{col}) SEMANTIC: {msg}");
}

// ── Run pipeline and produce ./output on success ──────────────────────────────

fn run_file(src: &str, _src_path: &str) -> i32 {
    // ── Lex + Parse ──────────────────────────────────────────────────────────
    let program = match compile(src) {
        Ok(p) => p,
        Err(PipelineError::Lex(e)) => {
            let (line, col) = e.position();
            emit_lexical(line, col, &e.clean_message());
            return 1;
        }
        Err(PipelineError::Parse(e)) => {
            let (line, col) = e.position();
            emit_syntactic(line, col, &e.clean_message());
            return 2;
        }
    };

    // ── Semantic ─────────────────────────────────────────────────────────────
    let errors = check(&program);
    if !errors.is_empty() {
        // Most fundamental error first (all are semantic here)
        for e in &errors {
            let (line, col) = e.position();
            emit_semantic(line, col, &e.clean_message());
        }
        return 3;
    }

    // ── Produce ./output ─────────────────────────────────────────────────────
    // ./output is a shell script that re-runs the interpreter on the source.
    // The source is base64-encoded inside the script to avoid escaping issues.
    let encoded = base64_encode(src.as_bytes());
    let script = format!(
        "#!/bin/sh\n\
         _H=\"$(cd \"$(dirname \"$0\")\" && pwd)/hulk\"\n\
         _T=$(mktemp)\n\
         printf '%s' '{encoded}' | base64 -d > \"$_T\"\n\
         \"$_H\" --run-stdin < \"$_T\"\n\
         _E=$?\n\
         rm -f \"$_T\"\n\
         exit $_E\n"
    );

    match fs::write("./output", &script) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("(0,0) SYNTACTIC: cannot write ./output: {e}");
            return 2;
        }
    }
    // Make executable
    let _ = fs::set_permissions("./output", fs::Permissions::from_mode(0o755));

    0
}

/// Run source from stdin (used by ./output scripts). Does NOT produce ./output.
fn run_stdin() -> i32 {
    let mut src = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut src) {
        eprintln!("(0,0) LEXICAL: cannot read stdin: {e}");
        return 1;
    }
    interpret_source(&src)
}

/// Interpret source directly, printing output. Returns exit code.
fn interpret_source(src: &str) -> i32 {
    let program = match compile(src) {
        Ok(p) => p,
        Err(PipelineError::Lex(e)) => {
            let (line, col) = e.position();
            emit_lexical(line, col, &e.clean_message());
            return 1;
        }
        Err(PipelineError::Parse(e)) => {
            let (line, col) = e.position();
            emit_syntactic(line, col, &e.clean_message());
            return 2;
        }
    };

    let errors = check(&program);
    if !errors.is_empty() {
        for e in &errors {
            let (line, col) = e.position();
            emit_semantic(line, col, &e.clean_message());
        }
        return 3;
    }

    let mut interp = Interpreter::new(&program);
    match interp.run_program(&program) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("runtime error: {e}");
            1
        }
    }
}

// ── Minimal base64 encoder (no external deps) ─────────────────────────────────

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[((n >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((n >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARS[((n >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARS[(n & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

// ── REPL (kept for developer convenience) ─────────────────────────────────────

fn repl() {
    let stdin  = io::stdin();
    let stdout = io::stdout();

    eprintln!("HULK REPL — type HULK expressions, Ctrl-D to quit");

    let mut preamble = String::new();

    loop {
        print!("hulk> ");
        stdout.lock().flush().unwrap();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => { println!(); break; }
            Err(e) => { eprintln!("Read error: {e}"); break; }
            Ok(_) => {}
        }

        let trimmed = line.trim();
        if trimmed.is_empty()   { continue; }
        if trimmed == ":q" || trimmed == ":quit" { break; }

        if trimmed.starts_with("class ")    ||
           trimmed.starts_with("function ") ||
           trimmed.starts_with("type ")     ||
           trimmed.starts_with("protocol ")
        {
            let candidate = format!("{}{}\n", preamble, trimmed);
            match parse_program(&candidate) {
                Ok(_) => {
                    preamble.push_str(trimmed);
                    preamble.push('\n');
                    eprintln!("(declaration registered)");
                }
                Err(e) => {
                    let (line, col) = e.position();
                    emit_syntactic(line, col, &e.clean_message());
                }
            }
            continue;
        }

        let entry = if trimmed.ends_with(';') {
            trimmed.to_string()
        } else {
            format!("{};", trimmed)
        };
        let src = format!("{}{}", preamble, entry);
        interpret_source(&src);
    }
    eprintln!("bye!");
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let code = match args.as_slice() {
        [] => {
            eprintln!("Usage: hulk <file.hulk>");
            eprintln!("       hulk --run-stdin   (read source from stdin, interpret)");
            eprintln!("       hulk repl");
            0
        }
        [h] if h == "--help" || h == "-h" || h == "help" => {
            eprintln!("Usage: hulk <file.hulk>");
            eprintln!("       hulk --run-stdin   (read source from stdin, interpret)");
            eprintln!("       hulk repl");
            0
        }
        [cmd] if cmd == "--run-stdin" => run_stdin(),
        [cmd] if cmd == "repl" => { repl(); 0 }
        [file] => {
            let src = read_file(file);
            run_file(&src, file)
        }
        [file, rest @ ..] if !file.starts_with('-') => {
            // Accept extra flags after file (ignored for compatibility)
            let _ = rest;
            let src = read_file(file);
            run_file(&src, file)
        }
        _ => {
            eprintln!("Usage: hulk <file.hulk>");
            1
        }
    };

    if code != 0 {
        process::exit(code);
    }
}

fn read_file(path: &str) -> String {
    fs::read_to_string(Path::new(path)).unwrap_or_else(|e| {
        eprintln!("(0,0) LEXICAL: cannot read '{path}': {e}");
        process::exit(1);
    })
}
