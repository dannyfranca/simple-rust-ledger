# Simple Rust Ledger

A toy payments engine that processes transactions from CSV, handles disputes and chargebacks, and outputs client account balances.

Opted to keep it single threaded as the real bottleneck is file IO, not CPU.

## Usage

```bash
cargo run -- transactions.csv > accounts.csv
```

## Assumptions

1. **Disputes only on deposits** - Per spec, only deposits can be disputed (fraud scenario describes deposit reversals). Disputing a withdrawal is ignored.
2. **Locked/Frozen accounts** - Block deposits/withdrawals, but allow disputes, resolutions, and chargebacks on existing transactions.
3. **Negative balances** - Can occur from chargebacks after partial withdrawals (not from normal operations).
4. **Precision** - Up to 4 decimal places, truncated (not rejected). No banker's rounding. Output always shows 4 decimals.
5. **Re-dispute** - After resolve/chargeback, cannot be re-disputed.
6. **Malformed/invalid lines** - Logged to stderr and keeps processing.

## Error Handling

- **Missing required columns**: Exits with error
- **Invalid operations** (e.g., insufficient funds): Silently ignored per spec

## Testing

See `samples/` for test cases covering: deposits, withdrawals, disputes, chargebacks, locked accounts, negative balances, edge cases, and malformed input.

## What if scaling to thousands of concurrent TCP streams?

First thoght was to use a Mutex or RwLock, but that would be inefficient due to not efficient lock contention.

Since each client_id is unique, we can use sharding instead, routing by `client_id % shard_count`, where each thread owns its own data, no locking needed.

My first option would be to use Tokio for I/O (accepting connections, reading bytes) and dedicated `std::thread`s for shard workers. Dedicated threads avoid work-stealing so each shard stays cache-friendly, Tokio could potentially be a better option if the shards are too unevenly distributed, only a proper benchmark would tell.
