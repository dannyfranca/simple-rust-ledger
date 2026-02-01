use std::env;
use std::fs::File;
use std::io::{self, BufReader, Write};
use std::process;

use simple_rust_ledger::domain::Ledger;
use simple_rust_ledger::parser::CsvParser;
use simple_rust_ledger::writer::{write_csv, OutputRecord};

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        return Err(format!(
            "Usage: {} <transactions.csv>\nExpected exactly 1 argument, got {}",
            args[0],
            args.len() - 1
        ));
    }

    let file_path = &args[1];

    let file =
        File::open(file_path).map_err(|e| format!("Failed to open '{}': {}", file_path, e))?;
    let reader = BufReader::new(file);

    let parser = CsvParser::new(reader)?;

    let mut ledger = Ledger::new();
    for result in parser {
        match result {
            Ok(record) => {
                ledger.process(
                    record.tx_type,
                    record.client_id,
                    record.tx_id,
                    record.amount,
                );
            }
            Err(e) => {
                let _ = writeln!(io::stderr(), "Warning: {}", e);
            }
        }
    }

    let stdout = io::stdout();
    let mut handle = stdout.lock();

    let records = ledger
        .accounts()
        .iter()
        .map(|(client_id, account)| OutputRecord::from_account(*client_id, account));

    write_csv(&mut handle, records).map_err(|e| format!("Failed to write output: {}", e))?;

    Ok(())
}
