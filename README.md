# Simple Rust Ledger

A toy payments engine that processes transactions from CSV, handles disputes and chargebacks, and outputs client account balances.

Opted to keep it single threaded as the real bottleneck is loading and parsing CSV, not processing transactions.

## Usage

```bash
cargo run -- transactions.csv > accounts.csv
```

## Assumptions

1. **Disputes only on deposits** - Per spec, only deposits can be disputed (fraud scenario describes deposit reversals). Disputing a withdrawal is ignored.
2. **Locked/Frozen accounts** - Block deposits/withdrawals, but allow disputes, resolutions, and chargebacks on existing transactions.
3. **Negative balances** - Can occur from chargebacks after partial withdrawals (not from normal operations).
4. **Precision** - Up to 4 decimal places, while more decimals are not expected, the library `rust_decimal` handles banker's rounding.
5. **Re-dispute** - After resolve/chargeback, cannot be re-disputed.
6. **Malformed/invalid lines** - Logged to stderr and keeps processing.

## Design Decisions

- Exact one CLI argument: Exit with an error message otherwise
- Missing columns lead to exit with error message, while extra columns are ignored
- No floating points: use the `rust_decimal` crate
- Serialize consistently 4 decimal places in the output CSV. Custom serializer with `rust_decimal` and `serde`
- Newtype pattern for Client IDs and transaction IDs (u16 and u32 respectively)
- Use single thread as the bottleneck is file IO and parsing, not CPU.
- Decouple the data stream from file IO, allowing other data sources to be implemented
- Use a Transaction enum rather than typestate to keep the code simple (readability over correctness for this simple project)
- Idempotency: Do not process the same withdrawal/deposit more than once (use a HashSet of tx IDs)
- Keep track of deposits in a HashMap due to disputes
- Core Domain with pure Rust
- Application Layer connecting the domain logic to the data stream
- CLI Layer as an executable interface

## Error Handling

- **Missing required columns**: Exits with error
- **Invalid operations** (e.g., insufficient funds): Silently ignored per spec

## Testing

Run `cargo test` for comprehensive coverage including: deposits, withdrawals, disputes, chargebacks, locked accounts, negative balances, edge cases, and malformed input. Unit tests are colocated in `src/` modules; integration tests are in `tests/integration.rs`.

## Stress Testing

Generate large datasets to stress test the engine:

```bash
# Generate and pipe 500k transactions directly (default is 0 errors)
cargo run --release --example stress_generator -- -n 500000 | cargo run --release -- /dev/stdin > output.csv

# To include corrupted lines (e.g., 5% error rate):
cargo run --release --example stress_generator -- -n 500000 -e 5 | cargo run --release -- /dev/stdin > output.csv
```

Generator options: `cargo run --example stress_generator -- --help`

## What if scaling to thousands of concurrent TCP streams?

First thought was to use a Mutex or RwLock, but that would be inefficient due to lock contention.

Since each client_id is unique, we can use sharding instead, routing by `client_id % shard_count`, where each thread owns its own data, no locking needed.

My first option would be to use Tokio for I/O (accepting connections, reading bytes) and dedicated `std::thread`s for shard workers. Dedicated threads avoid work-stealing so each shard stays cache-friendly. Tokio could be better if shards are unevenly distributed—only a proper benchmark would tell.

For final output, use `std::thread::scope` with channels: each shard sends its results through an `mpsc` channel, and a coordinator collects them. Rust's ownership guarantees senders are dropped when threads complete, safely signaling when all results are in. Since client IDs are unique per shard, no deduplication needed—just concatenation.
