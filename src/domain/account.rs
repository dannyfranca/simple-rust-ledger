use crate::domain::types::Amount;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub available: Amount,
    pub held: Amount,
    pub locked: bool,
}

impl Default for Account {
    fn default() -> Self {
        Self::new()
    }
}

impl Account {
    pub fn new() -> Self {
        Account {
            available: Amount::ZERO,
            held: Amount::ZERO,
            locked: false,
        }
    }

    pub fn total(&self) -> Amount {
        self.available + self.held
    }

    pub fn deposit(&mut self, amount: Amount) -> bool {
        if self.locked {
            return false;
        }
        self.available += amount;
        true
    }

    pub fn withdraw(&mut self, amount: Amount) -> bool {
        if self.locked {
            return false;
        }
        if self.available < amount {
            return false;
        }
        self.available -= amount;
        true
    }

    pub fn hold(&mut self, amount: Amount) {
        self.available -= amount;
        self.held += amount;
    }

    pub fn release(&mut self, amount: Amount) {
        self.held -= amount;
        self.available += amount;
    }

    pub fn chargeback(&mut self, amount: Amount) {
        self.held -= amount;
        self.locked = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn amount(s: &str) -> Amount {
        Amount::from_str_truncate(s).unwrap()
    }

    #[test]
    fn test_deposit_increases_available_and_total() {
        let mut account = Account::new();
        assert!(account.deposit(amount("100")));
        assert_eq!(account.available, amount("100"));
        assert_eq!(account.total(), amount("100"));
    }

    #[test]
    fn test_withdrawal_decreases_available_and_total() {
        let mut account = Account::new();
        account.deposit(amount("100"));
        assert!(account.withdraw(amount("30")));
        assert_eq!(account.available, amount("70"));
        assert_eq!(account.total(), amount("70"));
    }

    #[test]
    fn test_withdrawal_fails_insufficient_funds() {
        let mut account = Account::new();
        account.deposit(amount("50"));
        assert!(!account.withdraw(amount("100")));
        assert_eq!(account.available, amount("50"));
    }

    #[test]
    fn test_withdrawal_exact_amount() {
        let mut account = Account::new();
        account.deposit(amount("50"));
        assert!(account.withdraw(amount("50")));
        assert_eq!(account.available, amount("0"));
    }

    #[test]
    fn test_hold_moves_available_to_held() {
        let mut account = Account::new();
        account.deposit(amount("100"));
        account.hold(amount("40"));
        assert_eq!(account.available, amount("60"));
        assert_eq!(account.held, amount("40"));
        assert_eq!(account.total(), amount("100"));
    }

    #[test]
    fn test_release_moves_held_to_available() {
        let mut account = Account::new();
        account.deposit(amount("100"));
        account.hold(amount("40"));
        account.release(amount("40"));
        assert_eq!(account.available, amount("100"));
        assert_eq!(account.held, amount("0"));
    }

    #[test]
    fn test_chargeback_reduces_held_and_total_and_locks() {
        let mut account = Account::new();
        account.deposit(amount("100"));
        account.hold(amount("100"));
        account.chargeback(amount("100"));
        assert_eq!(account.available, amount("0"));
        assert_eq!(account.held, amount("0"));
        assert_eq!(account.total(), amount("0"));
        assert!(account.locked);
    }

    #[test]
    fn test_invariant_total_equals_available_plus_held() {
        let mut account = Account::new();
        account.deposit(amount("100"));
        assert_eq!(account.total(), account.available + account.held);

        account.hold(amount("30"));
        assert_eq!(account.total(), account.available + account.held);

        account.release(amount("10"));
        assert_eq!(account.total(), account.available + account.held);
    }

    #[test]
    fn test_locked_account_blocks_deposit() {
        let mut account = Account::new();
        account.locked = true;
        assert!(!account.deposit(amount("100")));
        assert_eq!(account.available, amount("0"));
    }

    #[test]
    fn test_locked_account_blocks_withdrawal() {
        let mut account = Account::new();
        account.deposit(amount("100"));
        account.locked = true;
        assert!(!account.withdraw(amount("50")));
        assert_eq!(account.available, amount("100"));
    }

    #[test]
    fn test_negative_balance_from_chargeback() {
        let mut account = Account::new();
        account.deposit(amount("100"));
        account.withdraw(amount("80"));
        account.hold(amount("100"));
        assert_eq!(account.available, amount("-80"));
        assert_eq!(account.held, amount("100"));
        account.chargeback(amount("100"));
        assert_eq!(account.available, amount("-80"));
        assert_eq!(account.held, amount("0"));
        assert_eq!(account.total(), amount("-80"));
        assert!(account.locked);
    }
}
