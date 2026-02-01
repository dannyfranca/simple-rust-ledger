use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClientId(pub u16);

impl fmt::Display for ClientId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TransactionId(pub u32);

impl fmt::Display for TransactionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Decimal amount with up to 4 decimal places precision
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Amount(pub Decimal);

impl Amount {
    pub const ZERO: Amount = Amount(Decimal::ZERO);

    pub fn new(value: Decimal) -> Self {
        Amount(value.round_dp(4))
    }

    pub fn from_str_truncate(s: &str) -> Result<Self, rust_decimal::Error> {
        let decimal = Decimal::from_str(s.trim())?;
        Ok(Self::new(decimal))
    }

    pub fn is_negative(&self) -> bool {
        self.0 < Decimal::ZERO
    }

    pub fn is_zero(&self) -> bool {
        self.0 == Decimal::ZERO
    }
}

impl std::ops::Add for Amount {
    type Output = Amount;
    fn add(self, rhs: Self) -> Self::Output {
        Amount(self.0 + rhs.0)
    }
}

impl std::ops::Sub for Amount {
    type Output = Amount;
    fn sub(self, rhs: Self) -> Self::Output {
        Amount(self.0 - rhs.0)
    }
}

impl std::ops::AddAssign for Amount {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl std::ops::SubAssign for Amount {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.4}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

impl FromStr for TransactionType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "deposit" => Ok(TransactionType::Deposit),
            "withdrawal" => Ok(TransactionType::Withdrawal),
            "dispute" => Ok(TransactionType::Dispute),
            "resolve" => Ok(TransactionType::Resolve),
            "chargeback" => Ok(TransactionType::Chargeback),
            _ => Err(()),
        }
    }
}

/// State of a stored transaction (for dispute tracking)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransactionState {
    #[default]
    None,
    Disputed,
    Resolved,
    ChargedBack,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amount_truncates_to_4_decimals() {
        let amount = Amount::from_str_truncate("1.23456").expect("failed to parse amount");
        assert_eq!(
            amount.0,
            Decimal::from_str("1.2346").expect("failed to parse decimal")
        );
    }

    #[test]
    fn test_amount_parses_with_whitespace() {
        let amount = Amount::from_str_truncate("  100.5  ").expect("failed to parse amount");
        assert_eq!(
            amount.0,
            Decimal::from_str("100.5").expect("failed to parse decimal")
        );
    }

    #[test]
    fn test_amount_display_4_decimals() {
        let amount = Amount::from_str_truncate("1.5").expect("failed to parse amount");
        assert_eq!(format!("{}", amount), "1.5000");
    }

    #[test]
    fn test_transaction_type_parsing() {
        assert_eq!(
            TransactionType::from_str("deposit"),
            Ok(TransactionType::Deposit)
        );
        assert_eq!(
            TransactionType::from_str(" WITHDRAWAL "),
            Ok(TransactionType::Withdrawal)
        );
        assert_eq!(
            TransactionType::from_str("Dispute"),
            Ok(TransactionType::Dispute)
        );
        assert!(TransactionType::from_str("invalid").is_err());
    }

    #[test]
    fn test_client_id_max() {
        let client = ClientId(u16::MAX);
        assert_eq!(client.0, 65535);
    }

    #[test]
    fn test_transaction_id_max() {
        let tx = TransactionId(u32::MAX);
        assert_eq!(tx.0, 4294967295);
    }
}
