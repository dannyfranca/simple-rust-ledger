use std::io::Write;

use crate::domain::types::{Amount, ClientId};
use crate::domain::Account;

pub struct OutputRecord {
    pub client: ClientId,
    pub available: Amount,
    pub held: Amount,
    pub total: Amount,
    pub locked: bool,
}

impl OutputRecord {
    pub fn from_account(client: ClientId, account: &Account) -> Self {
        OutputRecord {
            client,
            available: account.available,
            held: account.held,
            total: account.total(),
            locked: account.locked,
        }
    }
}

pub fn write_csv<W: Write>(
    writer: &mut W,
    records: impl Iterator<Item = OutputRecord>,
) -> std::io::Result<()> {
    writeln!(writer, "client,available,held,total,locked")?;

    for record in records {
        writeln!(
            writer,
            "{},{},{},{},{}",
            record.client, record.available, record.held, record.total, record.locked
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn amount(s: &str) -> Amount {
        Amount::from_str_rounded(s).expect("failed to parse amount")
    }

    #[test]
    fn test_output_4_decimal_precision() {
        let mut output = Vec::new();
        let records = vec![OutputRecord {
            client: ClientId(1),
            available: amount("1.5"),
            held: amount("0"),
            total: amount("1.5"),
            locked: false,
        }];
        write_csv(&mut output, records.into_iter()).expect("failed to write CSV");
        let csv = String::from_utf8(output).expect("output should be valid UTF-8");
        assert!(csv.contains("1.5000"));
        assert!(csv.contains("0.0000"));
    }

    #[test]
    fn test_output_column_order() {
        let mut output = Vec::new();
        let records = vec![OutputRecord {
            client: ClientId(1),
            available: amount("100"),
            held: amount("50"),
            total: amount("150"),
            locked: true,
        }];
        write_csv(&mut output, records.into_iter()).expect("failed to write CSV");
        let csv = String::from_utf8(output).expect("output should be valid UTF-8");
        let lines: Vec<_> = csv.lines().collect();
        assert_eq!(lines[0], "client,available,held,total,locked");
        assert_eq!(lines[1], "1,100.0000,50.0000,150.0000,true");
    }

    #[test]
    fn test_output_boolean_lowercase() {
        let mut output = Vec::new();
        let records = vec![
            OutputRecord {
                client: ClientId(1),
                available: amount("0"),
                held: amount("0"),
                total: amount("0"),
                locked: true,
            },
            OutputRecord {
                client: ClientId(2),
                available: amount("0"),
                held: amount("0"),
                total: amount("0"),
                locked: false,
            },
        ];
        write_csv(&mut output, records.into_iter()).expect("failed to write CSV");
        let csv = String::from_utf8(output).expect("output should be valid UTF-8");
        assert!(csv.contains(",true"));
        assert!(csv.contains(",false"));
        assert!(!csv.contains(",True"));
        assert!(!csv.contains(",False"));
    }

    #[test]
    fn test_output_negative_balance() {
        let mut output = Vec::new();
        let records = vec![OutputRecord {
            client: ClientId(1),
            available: amount("-80"),
            held: amount("0"),
            total: amount("-80"),
            locked: true,
        }];
        write_csv(&mut output, records.into_iter()).expect("failed to write CSV");
        let csv = String::from_utf8(output).expect("output should be valid UTF-8");
        assert!(csv.contains("-80.0000"));
    }

    #[test]
    fn test_output_empty_records() {
        let mut output = Vec::new();
        let records: Vec<OutputRecord> = vec![];
        write_csv(&mut output, records.into_iter()).expect("failed to write CSV");
        let csv = String::from_utf8(output).expect("output should be valid UTF-8");
        assert_eq!(csv, "client,available,held,total,locked\n");
    }

    #[test]
    fn test_from_account() {
        let mut account = Account::new();
        account.deposit(amount("100"));
        account.hold(amount("30"));

        let record = OutputRecord::from_account(ClientId(5), &account);
        assert_eq!(record.client, ClientId(5));
        assert_eq!(record.available, amount("70"));
        assert_eq!(record.held, amount("30"));
        assert_eq!(record.total, amount("100"));
        assert!(!record.locked);
    }

    #[test]
    fn test_output_no_trailing_whitespace() {
        let mut output = Vec::new();
        let records = vec![
            OutputRecord {
                client: ClientId(1),
                available: amount("100"),
                held: amount("0"),
                total: amount("100"),
                locked: false,
            },
            OutputRecord {
                client: ClientId(2),
                available: amount("50"),
                held: amount("25"),
                total: amount("75"),
                locked: true,
            },
        ];
        write_csv(&mut output, records.into_iter()).expect("failed to write CSV");
        let csv = String::from_utf8(output).expect("output should be valid UTF-8");
        for line in csv.lines() {
            assert!(
                !line.ends_with(' '),
                "Line has trailing whitespace: {:?}",
                line
            );
            assert!(!line.ends_with('\t'), "Line has trailing tab: {:?}", line);
        }
    }

    #[test]
    fn test_output_unix_newlines() {
        let mut output = Vec::new();
        let records = vec![OutputRecord {
            client: ClientId(1),
            available: amount("100"),
            held: amount("0"),
            total: amount("100"),
            locked: false,
        }];
        write_csv(&mut output, records.into_iter()).expect("failed to write CSV");
        let csv = String::from_utf8(output).expect("output should be valid UTF-8");
        // Should not contain CRLF
        assert!(!csv.contains("\r\n"), "Output contains CRLF instead of LF");
        // Should contain at least one newline
        assert!(csv.contains('\n'), "Output has no newlines");
    }
}
