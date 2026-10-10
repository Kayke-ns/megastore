mod cli;

use std::env;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("demo");
    match command {
        "demo" => cli::run_demo(),
        "interativo" | "repl" => cli::run_repl(),
        "bench" => cli::run_bench(&args[1..]),
        "ajuda" | "help" | "--help" | "-h" => cli::print_usage(),
        other => {
            eprintln!("Comando desconhecido: {other}\n");
            cli::print_usage();
            std::process::exit(2);
        }
    }
}
