use std::collections::HashMap;
use std::io::Cursor;

use simple_rust_ledger::domain::types::{Amount, ClientId};
use simple_rust_ledger::domain::Ledger;
use simple_rust_ledger::parser::CsvParser;
use simple_rust_ledger::writer::{write_csv, OutputRecord};

fn amount(s: &str) -> Amount {
    Amount::from_str_truncate(s).expect("failed to parse amount")
}

/// Helper to run a CSV through the ledger and get structured output
fn process_csv(input: &str) -> HashMap<ClientId, (Amount, Amount, Amount, bool)> {
    let cursor = Cursor::new(input);
    let parser = CsvParser::new(cursor).expect("failed to create CSV parser");

    let mut ledger = Ledger::new();
    for result in parser {
        if let Ok(record) = result {
            ledger.process(
                record.tx_type,
                record.client_id,
                record.tx_id,
                record.amount,
            );
        }
    }

    ledger
        .accounts()
        .iter()
        .map(|(client_id, account)| {
            (
                *client_id,
                (
                    account.available,
                    account.held,
                    account.total(),
                    account.locked,
                ),
            )
        })
        .collect()
}

fn get_csv_output(input: &str) -> String {
    let cursor = Cursor::new(input);
    let parser = CsvParser::new(cursor).expect("failed to create CSV parser");

    let mut ledger = Ledger::new();
    for result in parser {
        if let Ok(record) = result {
            ledger.process(
                record.tx_type,
                record.client_id,
                record.tx_id,
                record.amount,
            );
        }
    }

    let mut output = Vec::new();
    let records = ledger
        .accounts()
        .iter()
        .map(|(client_id, account)| OutputRecord::from_account(*client_id, account));
    write_csv(&mut output, records).expect("failed to write CSV output");

    String::from_utf8(output).expect("output should be valid UTF-8")
}

#[test]
fn test_basic_deposit() {
    let input = "type,client,tx,amount\ndeposit,1,1,100.0\n";
    let accounts = process_csv(input);

    assert_eq!(accounts.len(), 1);
    let (available, held, total, locked) = &accounts[&ClientId(1)];
    assert_eq!(*available, amount("100"));
    assert_eq!(*held, amount("0"));
    assert_eq!(*total, amount("100"));
    assert!(!locked);
}

#[test]
fn test_happy_path_deposit_withdrawal() {
    let input = r#"type,client,tx,amount
deposit,1,1,100.0
deposit,2,2,50.0
withdrawal,1,3,25.0
"#;
    let accounts = process_csv(input);

    assert_eq!(accounts.len(), 2);
    assert_eq!(accounts[&ClientId(1)].0, amount("75")); // 100 - 25
    assert_eq!(accounts[&ClientId(2)].0, amount("50"));
}

#[test]
fn test_dispute_resolve_cycle() {
    let input = r#"type,client,tx,amount
deposit,1,1,100.0
dispute,1,1,
resolve,1,1,
"#;
    let accounts = process_csv(input);

    let (available, held, total, locked) = &accounts[&ClientId(1)];
    assert_eq!(*available, amount("100"));
    assert_eq!(*held, amount("0"));
    assert_eq!(*total, amount("100"));
    assert!(!locked);
}

#[test]
fn test_chargeback_locks_account() {
    let input = r#"type,client,tx,amount
deposit,1,1,100.0
dispute,1,1,
chargeback,1,1,
"#;
    let accounts = process_csv(input);

    let (available, held, total, locked) = &accounts[&ClientId(1)];
    assert_eq!(*available, amount("0"));
    assert_eq!(*held, amount("0"));
    assert_eq!(*total, amount("0"));
    assert!(locked);
}

#[test]
fn test_negative_balance_from_chargeback() {
    let input = r#"type,client,tx,amount
deposit,1,1,100.0
withdrawal,1,2,80.0
dispute,1,1,
chargeback,1,1,
"#;
    let accounts = process_csv(input);

    let (available, held, total, locked) = &accounts[&ClientId(1)];
    // After deposit: available=100, After withdrawal: available=20
    // After dispute: available=-80 (20-100), held=100
    // After chargeback: available=-80, held=0, total=-80
    assert_eq!(*available, amount("-80"));
    assert_eq!(*held, amount("0"));
    assert_eq!(*total, amount("-80"));
    assert!(locked);
}

#[test]
fn test_locked_account_blocks_operations() {
    let input = r#"type,client,tx,amount
deposit,1,1,100.0
dispute,1,1,
chargeback,1,1,
deposit,1,2,50.0
withdrawal,1,3,10.0
"#;
    let accounts = process_csv(input);

    let (available, _held, total, locked) = &accounts[&ClientId(1)];
    // Deposit and withdrawal after lock should be ignored
    assert_eq!(*available, amount("0"));
    assert_eq!(*total, amount("0"));
    assert!(locked);
}

#[test]
fn test_edge_cases() {
    let input = r#"type,client,tx,amount
deposit,65535,4294967295,999999999.9999
deposit,1,1,1.2345
deposit , 2 , 2 , 0.0001
"#;
    let accounts = process_csv(input);

    assert_eq!(accounts.len(), 3);
    assert_eq!(accounts[&ClientId(65535)].0, amount("999999999.9999"));
    assert_eq!(accounts[&ClientId(1)].0, amount("1.2345"));
    assert_eq!(accounts[&ClientId(2)].0, amount("0.0001"));
}

#[test]
fn test_malformed_lines_skipped() {
    let input = r#"type,client,tx,amount
deposit,1,1,100.0
transfer,1,2,50.0
deposit,abc,3,10.0
deposit,1,4,-5.0
deposit,1,5,
withdrawal,1,6,20.0
"#;
    let accounts = process_csv(input);

    // Only valid deposit and withdrawal should be processed
    // deposit,1,1,100.0 succeeds
    // withdrawal,1,6,20.0 succeeds
    assert_eq!(accounts[&ClientId(1)].0, amount("80")); // 100 - 20
}

#[test]
fn test_output_format() {
    let input = "type,client,tx,amount\ndeposit,1,1,1.5\n";
    let output = get_csv_output(input);

    // Check header
    assert!(output.starts_with("client,available,held,total,locked\n"));

    // Check 4 decimal precision
    assert!(output.contains("1.5000"));

    // Check boolean is lowercase
    assert!(output.contains(",false"));
}

#[test]
fn test_multiple_clients_isolated() {
    let input = r#"type,client,tx,amount
deposit,1,1,100.0
deposit,2,2,200.0
dispute,1,1,
chargeback,1,1,
deposit,2,3,50.0
"#;
    let accounts = process_csv(input);

    // Client 1 locked with 0
    assert_eq!(accounts[&ClientId(1)].0, amount("0"));
    assert!(accounts[&ClientId(1)].3); // locked

    // Client 2 unaffected
    assert_eq!(accounts[&ClientId(2)].0, amount("250"));
    assert!(!accounts[&ClientId(2)].3); // not locked
}

#[test]
fn test_idempotent_duplicate_tx() {
    let input = r#"type,client,tx,amount
deposit,1,1,100.0
deposit,1,1,100.0
deposit,1,1,100.0
"#;
    let accounts = process_csv(input);

    // Only first deposit should count
    assert_eq!(accounts[&ClientId(1)].0, amount("100"));
}

#[test]
fn test_idempotent_duplicate_withdrawal_tx() {
    let input = r#"type,client,tx,amount
deposit,1,1,100.0
withdrawal,1,2,30.0
withdrawal,1,2,30.0
withdrawal,1,2,30.0
"#;
    let accounts = process_csv(input);

    // Only first withdrawal should count: 100 - 30 = 70
    assert_eq!(accounts[&ClientId(1)].0, amount("70"));
}

#[test]
fn test_idempotent_tx_id_shared_deposit_then_withdrawal() {
    let input = r#"type,client,tx,amount
deposit,1,1,100.0
withdrawal,1,1,50.0
"#;
    let accounts = process_csv(input);

    // Withdrawal with same tx ID as deposit should be blocked
    assert_eq!(accounts[&ClientId(1)].0, amount("100"));
}

#[test]
fn test_withdrawal_insufficient_funds() {
    let input = r#"type,client,tx,amount
deposit,1,1,50.0
withdrawal,1,2,100.0
"#;
    let accounts = process_csv(input);

    // Withdrawal should fail, balance unchanged
    assert_eq!(accounts[&ClientId(1)].0, amount("50"));
}

#[test]
fn test_dispute_wrong_client() {
    let input = r#"type,client,tx,amount
deposit,1,1,100.0
deposit,2,2,100.0
dispute,2,1,
"#;
    let accounts = process_csv(input);

    // Client 2 trying to dispute client 1's tx should fail
    assert_eq!(accounts[&ClientId(1)].0, amount("100"));
    assert_eq!(accounts[&ClientId(1)].1, amount("0")); // nothing held
    assert_eq!(accounts[&ClientId(2)].0, amount("100"));
}

#[test]
fn test_dispute_nonexistent_tx() {
    let input = r#"type,client,tx,amount
deposit,1,1,100.0
dispute,1,999,
"#;
    let accounts = process_csv(input);

    // Dispute of nonexistent tx should be ignored
    assert_eq!(accounts[&ClientId(1)].0, amount("100"));
    assert_eq!(accounts[&ClientId(1)].1, amount("0"));
}

#[test]
fn test_double_dispute_ignored() {
    let input = r#"type,client,tx,amount
deposit,1,1,100.0
dispute,1,1,
dispute,1,1,
"#;
    let accounts = process_csv(input);

    // Second dispute should be ignored, held should be 100 not 200
    assert_eq!(accounts[&ClientId(1)].0, amount("0"));
    assert_eq!(accounts[&ClientId(1)].1, amount("100"));
}

#[test]
fn test_empty_file() {
    let input = "type,client,tx,amount\n";
    let output = get_csv_output(input);

    // Should only have header
    assert_eq!(output, "client,available,held,total,locked\n");
}

#[test]
fn test_whitespace_handling() {
    let input = " type , client , tx , amount \n deposit , 1 , 1 , 100.0 \n";
    let accounts = process_csv(input);

    assert_eq!(accounts[&ClientId(1)].0, amount("100"));
}

#[test]
fn test_different_column_order() {
    let input = "amount,tx,client,type\n100.0,1,1,deposit\n";
    let accounts = process_csv(input);

    assert_eq!(accounts[&ClientId(1)].0, amount("100"));
}

#[test]
fn test_locked_account_allows_dispute_on_other_tx() {
    let input = r#"type,client,tx,amount
deposit,1,1,100.0
deposit,1,2,50.0
dispute,1,1,
chargeback,1,1,
dispute,1,2,
resolve,1,2,
"#;
    let accounts = process_csv(input);

    // Account locked from first chargeback
    // But dispute/resolve on second tx should still work
    assert!(accounts[&ClientId(1)].3); // locked
    assert_eq!(accounts[&ClientId(1)].0, amount("50")); // 50 from resolved dispute
    assert_eq!(accounts[&ClientId(1)].1, amount("0")); // nothing held after resolve
}

#[test]
fn test_duplicate_headers_uses_first() {
    // "type" appears twice - should use first column's value
    let input = "type,type,client,tx,amount\ndeposit,withdrawal,1,1,100\n";
    let accounts = process_csv(input);
    // If first "type" column is used, this is a deposit
    assert_eq!(accounts[&ClientId(1)].0, amount("100"));
}
