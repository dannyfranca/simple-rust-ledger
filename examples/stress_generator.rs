//! Stress test data generator for simple-rust-ledger
//!
//! Generates configurable CSV transaction data to stdout.
//! Uses a simple LCG (Linear Congruential Generator) for reproducible randomness.
//!
//! Usage:
//!   cargo run --example stress_generator -- [OPTIONS]
//!   cargo run --example stress_generator -- -n 10000 | cargo run -- /dev/stdin
//!
//! Options:
//!   -n, --transactions <N>  Number of transactions (default: 10000)
//!   -c, --clients <N>       Number of unique clients (default: 100)
//!   -e, --error-rate <N>    Percentage of corrupted lines 0-100 (default: 0)
//!   -s, --seed <N>          Random seed (default: 42)

use std::collections::HashMap;
use std::env;
use std::io::{self, BufWriter, Write};

/// Simple LCG (Linear Congruential Generator) for reproducible pseudo-random numbers
/// Parameters from Numerical Recipes
struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg { state: seed }
    }

    fn next(&mut self) -> u64 {
        // LCG constants from Numerical Recipes
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        self.state
    }

    fn next_u32(&mut self) -> u32 {
        (self.next() >> 32) as u32
    }

    fn next_range(&mut self, max: u32) -> u32 {
        if max == 0 {
            return 0;
        }
        self.next_u32() % max
    }

    fn next_bool(&mut self, probability_percent: u32) -> bool {
        self.next_range(100) < probability_percent
    }
}

#[derive(Clone, Copy)]
enum TxType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

impl TxType {
    fn as_str(&self) -> &'static str {
        match self {
            TxType::Deposit => "deposit",
            TxType::Withdrawal => "withdrawal",
            TxType::Dispute => "dispute",
            TxType::Resolve => "resolve",
            TxType::Chargeback => "chargeback",
        }
    }
}

/// Tracks deposit transaction IDs per client for generating valid disputes
struct ClientState {
    deposits: Vec<u32>, // tx_ids of deposits that can be disputed
    disputed: Vec<u32>, // tx_ids that are currently disputed
    balance: i64,       // approximate balance in cents for withdrawal validity
}

impl ClientState {
    fn new() -> Self {
        ClientState {
            deposits: Vec::new(),
            disputed: Vec::new(),
            balance: 0,
        }
    }
}

struct Config {
    transactions: u32,
    clients: u32,
    error_rate: u32,
    seed: u64,
}

impl Config {
    fn from_args() -> Result<Self, String> {
        let args: Vec<String> = env::args().collect();
        let mut config = Config {
            transactions: 10_000,
            clients: 100,
            error_rate: 0,
            seed: 42,
        };

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "-n" | "--transactions" => {
                    i += 1;
                    config.transactions = args
                        .get(i)
                        .ok_or("Missing value for --transactions")?
                        .parse()
                        .map_err(|_| "Invalid value for --transactions")?;
                }
                "-c" | "--clients" => {
                    i += 1;
                    config.clients = args
                        .get(i)
                        .ok_or("Missing value for --clients")?
                        .parse()
                        .map_err(|_| "Invalid value for --clients")?;
                }
                "-e" | "--error-rate" => {
                    i += 1;
                    config.error_rate = args
                        .get(i)
                        .ok_or("Missing value for --error-rate")?
                        .parse()
                        .map_err(|_| "Invalid value for --error-rate")?;
                }
                "-s" | "--seed" => {
                    i += 1;
                    config.seed = args
                        .get(i)
                        .ok_or("Missing value for --seed")?
                        .parse()
                        .map_err(|_| "Invalid value for --seed")?;
                }
                "-h" | "--help" => {
                    eprintln!("Usage: stress_generator [OPTIONS]");
                    eprintln!("  -n, --transactions <N>  Number of transactions (default: 10000)");
                    eprintln!("  -c, --clients <N>       Number of unique clients (default: 100)");
                    eprintln!("  -e, --error-rate <N>    Percentage of corrupted lines 0-100 (default: 0)");
                    eprintln!("  -s, --seed <N>          Random seed (default: 42)");
                    std::process::exit(0);
                }
                arg => return Err(format!("Unknown argument: {}", arg)),
            }
            i += 1;
        }

        // Clamp clients to u16 max
        if config.clients > 65535 {
            config.clients = 65535;
        }

        Ok(config)
    }
}

fn generate_corrupted_line(rng: &mut Lcg, tx_id: u32) -> String {
    let corruption_type = rng.next_range(8);
    match corruption_type {
        0 => format!("transfer,1,{},100.0", tx_id), // Invalid tx type
        1 => format!("credit,1,{},50.0", tx_id),    // Invalid tx type
        2 => format!("deposit,99999,{},100.0", tx_id), // Client ID overflow (>65535)
        3 => format!("deposit,1,9999999999,100.0"), // TX ID overflow (>u32::MAX)
        4 => format!("deposit,1,{},-50.0", tx_id),  // Negative amount
        5 => format!("deposit,1,{},", tx_id),       // Missing amount
        6 => format!("deposit,abc,{},100.0", tx_id), // Non-numeric client
        7 => format!("deposit,1,xyz,100.0"),        // Non-numeric tx_id
        _ => format!("invalid,line,data"),
    }
}

fn generate_amount(rng: &mut Lcg) -> String {
    // Generate amounts between 0.0001 and 999999.9999
    let whole = rng.next_range(1_000_000);
    let frac = rng.next_range(10000);
    format!("{}.{:04}", whole, frac)
}

fn main() -> Result<(), String> {
    let config = Config::from_args()?;

    let stdout = io::stdout();
    let mut writer = BufWriter::new(stdout.lock());

    let mut rng = Lcg::new(config.seed);
    let mut client_states: HashMap<u16, ClientState> = HashMap::new();
    let mut tx_id: u32 = 1;

    // Write header
    writeln!(writer, "type,client,tx,amount").map_err(|e| e.to_string())?;

    for _ in 0..config.transactions {
        // Decide if this is a corrupted line
        if rng.next_bool(config.error_rate) {
            let line = generate_corrupted_line(&mut rng, tx_id);
            writeln!(writer, "{}", line).map_err(|e| e.to_string())?;
            tx_id += 1;
            continue;
        }

        // Pick a random client
        let client_id = (rng.next_range(config.clients) + 1) as u16;
        let client = client_states
            .entry(client_id)
            .or_insert_with(ClientState::new);

        // Choose transaction type based on weighted distribution and client state
        let tx_type = {
            let roll = rng.next_range(100);
            if roll < 65 {
                // Increased deposits to 65% to build healthy balances
                TxType::Deposit
            } else if roll < 90 {
                // Withdrawals kept at 25%
                // Withdrawal - only if client has balance
                if client.balance > 0 {
                    TxType::Withdrawal
                } else {
                    TxType::Deposit
                }
            } else if roll < 96 {
                // Disputes reduced to 6%
                // Dispute - only if there are disputable deposits
                if !client.deposits.is_empty() {
                    TxType::Dispute
                } else {
                    TxType::Deposit
                }
            } else if roll < 98 {
                // Resolves reduced to 2%
                // Resolve - only if there are disputed transactions
                if !client.disputed.is_empty() {
                    TxType::Resolve
                } else {
                    TxType::Deposit
                }
            } else {
                // Chargebacks reduced to 2%
                // Chargeback - only if there are disputed transactions
                if !client.disputed.is_empty() {
                    TxType::Chargeback
                } else {
                    TxType::Deposit
                }
            }
        };

        let line = match tx_type {
            TxType::Deposit => {
                let amount = generate_amount(&mut rng);
                // Parse amount to track balance (approximate, in cents)
                if let Ok(value) = amount.parse::<f64>() {
                    client.balance += (value * 10000.0) as i64;
                }
                client.deposits.push(tx_id);
                format!("{},{},{},{}", tx_type.as_str(), client_id, tx_id, amount)
            }
            TxType::Withdrawal => {
                // Generate a withdrawal that's likely valid (up to current balance)
                let max_amount = (client.balance as f64 / 10000.0).max(0.0);
                let withdraw = rng.next_range((max_amount * 100.0) as u32 + 1) as f64 / 100.0;
                client.balance -= (withdraw * 10000.0) as i64;
                format!(
                    "{},{},{},{:.4}",
                    tx_type.as_str(),
                    client_id,
                    tx_id,
                    withdraw
                )
            }
            TxType::Dispute => {
                // Pick a random deposit to dispute
                let idx = rng.next_range(client.deposits.len() as u32) as usize;
                let disputed_tx = client.deposits.remove(idx);
                client.disputed.push(disputed_tx);
                format!("{},{},{},", tx_type.as_str(), client_id, disputed_tx)
            }
            TxType::Resolve => {
                // Pick a random disputed tx to resolve
                let idx = rng.next_range(client.disputed.len() as u32) as usize;
                let resolved_tx = client.disputed.remove(idx);
                client.deposits.push(resolved_tx); // Can be disputed again
                format!("{},{},{},", tx_type.as_str(), client_id, resolved_tx)
            }
            TxType::Chargeback => {
                // Pick a random disputed tx to chargeback
                let idx = rng.next_range(client.disputed.len() as u32) as usize;
                let chargeback_tx = client.disputed.remove(idx);
                format!("{},{},{},", tx_type.as_str(), client_id, chargeback_tx)
            }
        };

        writeln!(writer, "{}", line).map_err(|e| e.to_string())?;
        tx_id += 1;
    }

    writer.flush().map_err(|e| e.to_string())?;
    Ok(())
}
