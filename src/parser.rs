use csv::ReaderBuilder;
use std::io::Read;

use crate::domain::types::{Amount, ClientId, TransactionId, TransactionType};

#[derive(Debug)]
pub struct InputRecord {
    pub tx_type: TransactionType,
    pub client_id: ClientId,
    pub tx_id: TransactionId,
    pub amount: Option<Amount>,
}

#[derive(Debug)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Line {}: {}", self.line, self.message)
    }
}

#[derive(Debug)]
struct ColumnIndices {
    type_idx: usize,
    client_idx: usize,
    tx_idx: usize,
    amount_idx: usize,
}

pub struct CsvParser<R: Read> {
    reader: csv::Reader<R>,
    line_number: usize,
    columns: ColumnIndices,
}

impl<R: Read> std::fmt::Debug for CsvParser<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CsvParser")
            .field("line_number", &self.line_number)
            .field("columns", &self.columns)
            .finish_non_exhaustive()
    }
}

impl<R: Read> CsvParser<R> {
    pub fn new(reader: R) -> Result<Self, String> {
        let mut csv_reader = ReaderBuilder::new()
            .flexible(true)
            .trim(csv::Trim::All)
            .has_headers(true)
            .from_reader(reader);

        let headers = csv_reader
            .headers()
            .map_err(|e| format!("Failed to read headers: {}", e))?
            .clone();

        let columns = Self::extract_column_indices(&headers)?;

        Ok(CsvParser {
            reader: csv_reader,
            line_number: 1,
            columns,
        })
    }

    fn extract_column_indices(headers: &csv::StringRecord) -> Result<ColumnIndices, String> {
        let find_col = |name: &str| -> Result<usize, String> {
            headers
                .iter()
                .position(|h| h.trim().eq_ignore_ascii_case(name))
                .ok_or_else(|| format!("Missing required column: '{}'", name))
        };

        Ok(ColumnIndices {
            type_idx: find_col("type")?,
            client_idx: find_col("client")?,
            tx_idx: find_col("tx")?,
            amount_idx: find_col("amount")?,
        })
    }

    pub fn next_record(&mut self) -> Option<Result<InputRecord, ParseError>> {
        let mut record = csv::StringRecord::new();

        self.line_number += 1;
        let current_line = self.line_number;

        match self.reader.read_record(&mut record) {
            Ok(true) => match self.parse_record(&record, current_line) {
                Ok(input) => Some(Ok(input)),
                Err(e) => Some(Err(e)),
            },
            Ok(false) => None,
            Err(e) => Some(Err(ParseError {
                line: current_line,
                message: format!("CSV error: {}", e),
            })),
        }
    }

    fn parse_record(
        &self,
        record: &csv::StringRecord,
        line: usize,
    ) -> Result<InputRecord, ParseError> {
        let tx_type_str = record.get(self.columns.type_idx).unwrap_or("").trim();
        let tx_type: TransactionType = tx_type_str.parse().map_err(|_| ParseError {
            line,
            message: format!("Unknown transaction type: '{}'", tx_type_str),
        })?;

        let client_str = record.get(self.columns.client_idx).unwrap_or("").trim();
        let client_id: u16 = client_str.parse().map_err(|_| ParseError {
            line,
            message: format!("Invalid client ID: '{}'", client_str),
        })?;

        let tx_str = record.get(self.columns.tx_idx).unwrap_or("").trim();
        let tx_id: u32 = tx_str.parse().map_err(|_| ParseError {
            line,
            message: format!("Invalid transaction ID: '{}'", tx_str),
        })?;

        let amount_str = record.get(self.columns.amount_idx).unwrap_or("").trim();
        let amount = if amount_str.is_empty() {
            None
        } else {
            let parsed = Amount::from_str_truncate(amount_str).map_err(|_| ParseError {
                line,
                message: format!("Invalid amount: '{}'", amount_str),
            })?;
            if parsed.is_negative() {
                return Err(ParseError {
                    line,
                    message: format!("Negative amount not allowed: '{}'", amount_str),
                });
            }
            Some(parsed)
        };

        match tx_type {
            TransactionType::Deposit | TransactionType::Withdrawal => {
                if amount.is_none() {
                    return Err(ParseError {
                        line,
                        message: "Deposit/withdrawal requires amount".to_string(),
                    });
                }
            }
            _ => {}
        }

        Ok(InputRecord {
            tx_type,
            client_id: ClientId(client_id),
            tx_id: TransactionId(tx_id),
            amount,
        })
    }
}

impl<R: Read> Iterator for CsvParser<R> {
    type Item = Result<InputRecord, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_record()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn parse_csv(input: &str) -> Vec<Result<InputRecord, ParseError>> {
        let cursor = Cursor::new(input);
        let parser = CsvParser::new(cursor).unwrap();
        parser.collect()
    }

    fn amount(s: &str) -> Amount {
        Amount::from_str_truncate(s).unwrap()
    }

    #[test]
    fn test_parse_standard_csv() {
        let input = "type,client,tx,amount\ndeposit,1,1,100.0\n";
        let results: Vec<_> = parse_csv(input);
        assert_eq!(results.len(), 1);
        let record = results[0].as_ref().unwrap();
        assert_eq!(record.tx_type, TransactionType::Deposit);
        assert_eq!(record.client_id, ClientId(1));
        assert_eq!(record.tx_id, TransactionId(1));
        assert_eq!(record.amount.unwrap(), amount("100"));
    }

    #[test]
    fn test_parse_whitespace_in_headers_and_values() {
        let input = " type , client , tx , amount \n deposit , 1 , 1 , 100.0 \n";
        let results: Vec<_> = parse_csv(input);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
    }

    #[test]
    fn test_parse_different_column_order() {
        let input = "amount,tx,client,type\n100.0,1,1,deposit\n";
        let results: Vec<_> = parse_csv(input);
        assert_eq!(results.len(), 1);
        let record = results[0].as_ref().unwrap();
        assert_eq!(record.tx_type, TransactionType::Deposit);
        assert_eq!(record.client_id, ClientId(1));
    }

    #[test]
    fn test_parse_extra_columns_ignored() {
        let input = "type,client,tx,amount,extra\ndeposit,1,1,100.0,ignored\n";
        let results: Vec<_> = parse_csv(input);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
    }

    #[test]
    fn test_parse_dispute_no_amount() {
        let input = "type,client,tx,amount\ndispute,1,1,\n";
        let results: Vec<_> = parse_csv(input);
        assert_eq!(results.len(), 1);
        let record = results[0].as_ref().unwrap();
        assert_eq!(record.tx_type, TransactionType::Dispute);
        assert!(record.amount.is_none());
    }

    #[test]
    fn test_parse_max_precision_4_decimals() {
        let input = "type,client,tx,amount\ndeposit,1,1,1.2345\n";
        let results: Vec<_> = parse_csv(input);
        let record = results[0].as_ref().unwrap();
        assert_eq!(record.amount.unwrap(), amount("1.2345"));
    }

    #[test]
    fn test_truncate_excess_precision() {
        let input = "type,client,tx,amount\ndeposit,1,1,1.23456\n";
        let results: Vec<_> = parse_csv(input);
        let record = results[0].as_ref().unwrap();
        // Should truncate to 4 decimals (with rounding)
        assert_eq!(record.amount.unwrap(), amount("1.2346"));
    }

    #[test]
    fn test_skip_negative_amount() {
        let input = "type,client,tx,amount\ndeposit,1,1,-5.0\n";
        let results: Vec<_> = parse_csv(input);
        assert!(results[0].is_err());
        assert!(results[0]
            .as_ref()
            .unwrap_err()
            .message
            .contains("Negative"));
    }

    #[test]
    fn test_skip_overflow_client_id() {
        let input = "type,client,tx,amount\ndeposit,65536,1,100\n";
        let results: Vec<_> = parse_csv(input);
        assert!(results[0].is_err());
    }

    #[test]
    fn test_skip_overflow_tx_id() {
        let input = "type,client,tx,amount\ndeposit,1,4294967296,100\n";
        let results: Vec<_> = parse_csv(input);
        assert!(results[0].is_err());
    }

    #[test]
    fn test_max_client_id() {
        let input = "type,client,tx,amount\ndeposit,65535,1,100\n";
        let results: Vec<_> = parse_csv(input);
        assert!(results[0].is_ok());
        assert_eq!(results[0].as_ref().unwrap().client_id, ClientId(65535));
    }

    #[test]
    fn test_max_tx_id() {
        let input = "type,client,tx,amount\ndeposit,1,4294967295,100\n";
        let results: Vec<_> = parse_csv(input);
        assert!(results[0].is_ok());
        assert_eq!(
            results[0].as_ref().unwrap().tx_id,
            TransactionId(4294967295)
        );
    }

    #[test]
    fn test_leading_zeros() {
        let input = "type,client,tx,amount\ndeposit,001,001,001.0\n";
        let results: Vec<_> = parse_csv(input);
        let record = results[0].as_ref().unwrap();
        assert_eq!(record.client_id, ClientId(1));
        assert_eq!(record.tx_id, TransactionId(1));
    }

    #[test]
    fn test_unknown_transaction_type() {
        let input = "type,client,tx,amount\ntransfer,1,1,100\n";
        let results: Vec<_> = parse_csv(input);
        assert!(results[0].is_err());
    }

    #[test]
    fn test_non_numeric_client() {
        let input = "type,client,tx,amount\ndeposit,abc,1,100\n";
        let results: Vec<_> = parse_csv(input);
        assert!(results[0].is_err());
    }

    #[test]
    fn test_deposit_missing_amount() {
        let input = "type,client,tx,amount\ndeposit,1,1,\n";
        let results: Vec<_> = parse_csv(input);
        assert!(results[0].is_err());
        assert!(results[0]
            .as_ref()
            .unwrap_err()
            .message
            .contains("requires amount"));
    }

    #[test]
    fn test_quoted_values() {
        let input = "type,client,tx,amount\n\"deposit\",\"1\",\"1\",\"100\"\n";
        let results: Vec<_> = parse_csv(input);
        assert!(results[0].is_ok());
    }

    #[test]
    fn test_empty_file_after_headers() {
        let input = "type,client,tx,amount\n";
        let results: Vec<_> = parse_csv(input);
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_all_transaction_types() {
        let input = "type,client,tx,amount
deposit,1,1,100
withdrawal,1,2,50
dispute,1,1,
resolve,1,1,
chargeback,1,3,
";
        let results: Vec<_> = parse_csv(input);
        assert_eq!(results.len(), 5);
        assert!(results.iter().all(|r| r.is_ok()));
    }

    #[test]
    fn test_missing_column_fails_early() {
        let input = "type,client,tx\ndeposit,1,1\n";
        let cursor = Cursor::new(input);
        let result = CsvParser::new(cursor);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("amount"));
    }
}
