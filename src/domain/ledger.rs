use std::collections::{HashMap, HashSet};

use crate::domain::account::Account;
use crate::domain::types::{Amount, ClientId, TransactionId, TransactionState, TransactionType};

/// A stored deposit transaction for dispute tracking
#[derive(Debug, Clone)]
pub struct StoredTransaction {
    pub client_id: ClientId,
    pub amount: Amount,
    pub state: TransactionState,
}

/// Maintains client accounts and processes transactions.
#[derive(Debug, Default)]
pub struct Ledger {
    accounts: HashMap<ClientId, Account>,
    deposits: HashMap<TransactionId, StoredTransaction>,
    /// Tracks processed tx IDs for idempotency.
    processed_tx_ids: HashSet<TransactionId>,
}

impl Ledger {
    pub fn new() -> Self {
        Ledger {
            accounts: HashMap::new(),
            deposits: HashMap::new(),
            processed_tx_ids: HashSet::new(),
        }
    }

    fn get_or_create_account(&mut self, client_id: ClientId) -> &mut Account {
        self.accounts.entry(client_id).or_default()
    }

    pub fn get_account(&self, client_id: ClientId) -> Option<&Account> {
        self.accounts.get(&client_id)
    }

    pub fn accounts(&self) -> &HashMap<ClientId, Account> {
        &self.accounts
    }

    /// Returns true if the transaction was successfully processed.
    pub fn process(
        &mut self,
        tx_type: TransactionType,
        client_id: ClientId,
        tx_id: TransactionId,
        amount: Option<Amount>,
    ) -> bool {
        match tx_type {
            TransactionType::Deposit => self.process_deposit(client_id, tx_id, amount),
            TransactionType::Withdrawal => self.process_withdrawal(client_id, tx_id, amount),
            TransactionType::Dispute => self.process_dispute(client_id, tx_id),
            TransactionType::Resolve => self.process_resolve(client_id, tx_id),
            TransactionType::Chargeback => self.process_chargeback(client_id, tx_id),
        }
    }

    fn process_deposit(
        &mut self,
        client_id: ClientId,
        tx_id: TransactionId,
        amount: Option<Amount>,
    ) -> bool {
        let amount = match amount {
            Some(a) if !a.is_negative() => a,
            _ => return false,
        };

        if self.processed_tx_ids.contains(&tx_id) {
            return false;
        }

        let account = self.get_or_create_account(client_id);
        if !account.deposit(amount) {
            return false;
        }

        self.processed_tx_ids.insert(tx_id);
        self.deposits.insert(
            tx_id,
            StoredTransaction {
                client_id,
                amount,
                state: TransactionState::None,
            },
        );
        true
    }

    fn process_withdrawal(
        &mut self,
        client_id: ClientId,
        tx_id: TransactionId,
        amount: Option<Amount>,
    ) -> bool {
        let amount = match amount {
            Some(a) if !a.is_negative() => a,
            _ => return false,
        };

        if self.processed_tx_ids.contains(&tx_id) {
            return false;
        }

        let account = self.get_or_create_account(client_id);
        if !account.withdraw(amount) {
            return false;
        }

        self.processed_tx_ids.insert(tx_id);
        true
    }

    fn process_dispute(&mut self, client_id: ClientId, tx_id: TransactionId) -> bool {
        let stored = match self.deposits.get_mut(&tx_id) {
            Some(s) => s,
            None => return false,
        };

        if stored.client_id != client_id {
            return false;
        }

        if stored.state != TransactionState::None {
            return false;
        }

        let amount = stored.amount;
        stored.state = TransactionState::Disputed;

        let account = self.get_or_create_account(client_id);
        account.hold(amount);

        true
    }

    fn process_resolve(&mut self, client_id: ClientId, tx_id: TransactionId) -> bool {
        let stored = match self.deposits.get_mut(&tx_id) {
            Some(s) => s,
            None => return false,
        };

        if stored.client_id != client_id {
            return false;
        }

        if stored.state != TransactionState::Disputed {
            return false;
        }

        let amount = stored.amount;
        stored.state = TransactionState::Resolved;

        let account = self.get_or_create_account(client_id);
        account.release(amount);

        true
    }

    fn process_chargeback(&mut self, client_id: ClientId, tx_id: TransactionId) -> bool {
        let stored = match self.deposits.get_mut(&tx_id) {
            Some(s) => s,
            None => return false,
        };

        if stored.client_id != client_id {
            return false;
        }

        if stored.state != TransactionState::Disputed {
            return false;
        }

        let amount = stored.amount;
        stored.state = TransactionState::ChargedBack;

        let account = self.get_or_create_account(client_id);
        account.chargeback(amount);

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn amount(s: &str) -> Amount {
        Amount::from_str_truncate(s).expect("failed to parse amount")
    }

    fn client(id: u16) -> ClientId {
        ClientId(id)
    }

    fn tx(id: u32) -> TransactionId {
        TransactionId(id)
    }

    #[test]
    fn test_deposit_creates_account() {
        let mut ledger = Ledger::new();
        assert!(ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100"))
        ));
        let acc = ledger
            .get_account(client(1))
            .expect("client(1) account should exist after deposit");
        assert_eq!(acc.available, amount("100"));
        assert_eq!(acc.total(), amount("100"));
    }

    #[test]
    fn test_dispute_deposit_holds_funds() {
        let mut ledger = Ledger::new();
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        assert!(ledger.process(TransactionType::Dispute, client(1), tx(1), None));
        let acc = ledger
            .get_account(client(1))
            .expect("client(1) account should exist after dispute");
        assert_eq!(acc.available, amount("0"));
        assert_eq!(acc.held, amount("100"));
        assert_eq!(acc.total(), amount("100"));
    }

    #[test]
    fn test_resolve_releases_held_funds() {
        let mut ledger = Ledger::new();
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        ledger.process(TransactionType::Dispute, client(1), tx(1), None);
        assert!(ledger.process(TransactionType::Resolve, client(1), tx(1), None));
        let acc = ledger
            .get_account(client(1))
            .expect("client(1) account should exist after resolve");
        assert_eq!(acc.available, amount("100"));
        assert_eq!(acc.held, amount("0"));
    }

    #[test]
    fn test_chargeback_locks_account() {
        let mut ledger = Ledger::new();
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        ledger.process(TransactionType::Dispute, client(1), tx(1), None);
        assert!(ledger.process(TransactionType::Chargeback, client(1), tx(1), None));
        let acc = ledger
            .get_account(client(1))
            .expect("client(1) account should exist after chargeback");
        assert_eq!(acc.available, amount("0"));
        assert_eq!(acc.held, amount("0"));
        assert_eq!(acc.total(), amount("0"));
        assert!(acc.locked);
    }

    #[test]
    fn test_dispute_nonexistent_tx_ignored() {
        let mut ledger = Ledger::new();
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        assert!(!ledger.process(TransactionType::Dispute, client(1), tx(999), None));
    }

    #[test]
    fn test_dispute_wrong_client_ignored() {
        let mut ledger = Ledger::new();
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        // Client 2 tries to dispute client 1's transaction
        assert!(!ledger.process(TransactionType::Dispute, client(2), tx(1), None));
    }

    #[test]
    fn test_dispute_withdrawal_ignored() {
        let mut ledger = Ledger::new();
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        ledger.process(
            TransactionType::Withdrawal,
            client(1),
            tx(2),
            Some(amount("50")),
        );
        // Withdrawals aren't stored, so disputing tx(2) should fail
        assert!(!ledger.process(TransactionType::Dispute, client(1), tx(2), None));
    }

    #[test]
    fn test_double_dispute_ignored() {
        let mut ledger = Ledger::new();
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        assert!(ledger.process(TransactionType::Dispute, client(1), tx(1), None));
        assert!(!ledger.process(TransactionType::Dispute, client(1), tx(1), None));
    }

    #[test]
    fn test_locked_account_blocks_deposit_withdrawal() {
        let mut ledger = Ledger::new();
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        ledger.process(TransactionType::Dispute, client(1), tx(1), None);
        ledger.process(TransactionType::Chargeback, client(1), tx(1), None);

        // Account now locked
        assert!(!ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(2),
            Some(amount("50"))
        ));
        assert!(!ledger.process(
            TransactionType::Withdrawal,
            client(1),
            tx(3),
            Some(amount("10"))
        ));
    }

    #[test]
    fn test_locked_account_allows_dispute_resolve_chargeback() {
        let mut ledger = Ledger::new();
        // First deposit and lock via chargeback
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        ledger.process(TransactionType::Dispute, client(1), tx(1), None);
        ledger.process(TransactionType::Chargeback, client(1), tx(1), None);

        // Second deposit before lock (simulating this by manually adjusting)
        // Actually, we need to deposit before the lock happens
        // Let's test with a fresh scenario
        let mut ledger2 = Ledger::new();
        ledger2.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        ledger2.process(
            TransactionType::Deposit,
            client(1),
            tx(2),
            Some(amount("50")),
        );
        ledger2.process(TransactionType::Dispute, client(1), tx(1), None);
        ledger2.process(TransactionType::Chargeback, client(1), tx(1), None);
        // Account locked, but we can still dispute tx(2)
        assert!(ledger2.process(TransactionType::Dispute, client(1), tx(2), None));
        assert!(ledger2.process(TransactionType::Resolve, client(1), tx(2), None));
    }

    #[test]
    fn test_negative_balance_from_chargeback() {
        let mut ledger = Ledger::new();
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        ledger.process(
            TransactionType::Withdrawal,
            client(1),
            tx(2),
            Some(amount("80")),
        );
        ledger.process(TransactionType::Dispute, client(1), tx(1), None);
        ledger.process(TransactionType::Chargeback, client(1), tx(1), None);

        let acc = ledger
            .get_account(client(1))
            .expect("client(1) account should exist");
        // available should be -80 (20 - 100 = -80)
        assert_eq!(format!("{}", acc.available), "-80.0000");
        assert!(acc.locked);
    }

    #[test]
    fn test_idempotent_duplicate_tx_id() {
        let mut ledger = Ledger::new();
        assert!(ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100"))
        ));
        // Same tx ID again should be ignored
        assert!(!ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100"))
        ));
        let acc = ledger
            .get_account(client(1))
            .expect("client(1) account should exist");
        assert_eq!(acc.available, amount("100")); // Not 200
    }

    #[test]
    fn test_idempotent_duplicate_withdrawal_tx_id() {
        let mut ledger = Ledger::new();
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        assert!(ledger.process(
            TransactionType::Withdrawal,
            client(1),
            tx(2),
            Some(amount("30"))
        ));
        // Same withdrawal tx ID again should be ignored
        assert!(!ledger.process(
            TransactionType::Withdrawal,
            client(1),
            tx(2),
            Some(amount("30"))
        ));
        let acc = ledger
            .get_account(client(1))
            .expect("client(1) account should exist");
        assert_eq!(acc.available, amount("70")); // 100 - 30, not 100 - 60
    }

    #[test]
    fn test_idempotent_tx_id_shared_across_types() {
        // Same tx ID used for deposit, then attempted for withdrawal
        let mut ledger = Ledger::new();
        assert!(ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100"))
        ));
        // Withdrawal with same tx ID should be rejected
        assert!(!ledger.process(
            TransactionType::Withdrawal,
            client(1),
            tx(1),
            Some(amount("50"))
        ));
        let acc = ledger
            .get_account(client(1))
            .expect("client(1) account should exist");
        assert_eq!(acc.available, amount("100")); // No withdrawal happened
    }

    #[test]
    fn test_resolve_non_disputed_tx_ignored() {
        let mut ledger = Ledger::new();
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        // Try to resolve without disputing first
        assert!(!ledger.process(TransactionType::Resolve, client(1), tx(1), None));
    }

    #[test]
    fn test_chargeback_non_disputed_tx_ignored() {
        let mut ledger = Ledger::new();
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        // Try to chargeback without disputing first
        assert!(!ledger.process(TransactionType::Chargeback, client(1), tx(1), None));
    }

    #[test]
    fn test_re_dispute_after_resolve_ignored() {
        let mut ledger = Ledger::new();
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        ledger.process(TransactionType::Dispute, client(1), tx(1), None);
        ledger.process(TransactionType::Resolve, client(1), tx(1), None);
        // Try to dispute again
        assert!(!ledger.process(TransactionType::Dispute, client(1), tx(1), None));
    }

    #[test]
    fn test_re_dispute_after_chargeback_ignored() {
        let mut ledger = Ledger::new();
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        ledger.process(TransactionType::Dispute, client(1), tx(1), None);
        ledger.process(TransactionType::Chargeback, client(1), tx(1), None);
        // Try to dispute again
        assert!(!ledger.process(TransactionType::Dispute, client(1), tx(1), None));
    }

    #[test]
    fn test_multiple_clients_isolated() {
        let mut ledger = Ledger::new();
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        ledger.process(
            TransactionType::Deposit,
            client(2),
            tx(2),
            Some(amount("200")),
        );
        ledger.process(
            TransactionType::Withdrawal,
            client(1),
            tx(3),
            Some(amount("50")),
        );

        let acc1 = ledger
            .get_account(client(1))
            .expect("client(1) account should exist");
        let acc2 = ledger
            .get_account(client(2))
            .expect("client(2) account should exist");
        assert_eq!(acc1.available, amount("50"));
        assert_eq!(acc2.available, amount("200"));
    }

    #[test]
    fn test_zero_amount_deposit() {
        let mut ledger = Ledger::new();
        assert!(ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("0"))
        ));
        let acc = ledger
            .get_account(client(1))
            .expect("client(1) account should exist");
        assert_eq!(acc.available, amount("0"));
    }

    #[test]
    fn test_negative_amount_rejected() {
        let mut ledger = Ledger::new();
        let neg = Amount::new(rust_decimal::Decimal::new(-100, 0));
        assert!(!ledger.process(TransactionType::Deposit, client(1), tx(1), Some(neg)));
    }

    #[test]
    fn test_re_resolve_same_tx_ignored() {
        let mut ledger = Ledger::new();
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        ledger.process(TransactionType::Dispute, client(1), tx(1), None);
        assert!(ledger.process(TransactionType::Resolve, client(1), tx(1), None));
        // Second resolve should fail
        assert!(!ledger.process(TransactionType::Resolve, client(1), tx(1), None));
    }

    #[test]
    fn test_re_chargeback_same_tx_ignored() {
        let mut ledger = Ledger::new();
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        ledger.process(TransactionType::Dispute, client(1), tx(1), None);
        assert!(ledger.process(TransactionType::Chargeback, client(1), tx(1), None));
        // Second chargeback should fail
        assert!(!ledger.process(TransactionType::Chargeback, client(1), tx(1), None));
    }

    #[test]
    fn test_chargeback_then_resolve_ignored() {
        let mut ledger = Ledger::new();
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        ledger.process(TransactionType::Dispute, client(1), tx(1), None);
        ledger.process(TransactionType::Chargeback, client(1), tx(1), None);
        // Resolve after chargeback should fail
        assert!(!ledger.process(TransactionType::Resolve, client(1), tx(1), None));
    }

    #[test]
    fn test_resolve_wrong_client_ignored() {
        let mut ledger = Ledger::new();
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        ledger.process(TransactionType::Dispute, client(1), tx(1), None);
        // Client 2 tries to resolve client 1's disputed transaction
        assert!(!ledger.process(TransactionType::Resolve, client(2), tx(1), None));
        // Verify client 1's funds still held
        let acc = ledger
            .get_account(client(1))
            .expect("client(1) account should exist");
        assert_eq!(acc.held, amount("100"));
    }

    #[test]
    fn test_chargeback_wrong_client_ignored() {
        let mut ledger = Ledger::new();
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        ledger.process(TransactionType::Dispute, client(1), tx(1), None);
        // Client 2 tries to chargeback client 1's disputed transaction
        assert!(!ledger.process(TransactionType::Chargeback, client(2), tx(1), None));
        // Verify client 1's account not locked
        let acc = ledger
            .get_account(client(1))
            .expect("client(1) account should exist");
        assert!(!acc.locked);
    }

    #[test]
    fn test_zero_amount_withdrawal() {
        let mut ledger = Ledger::new();
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        // Zero withdrawal should succeed as no-op
        assert!(ledger.process(
            TransactionType::Withdrawal,
            client(1),
            tx(2),
            Some(amount("0"))
        ));
        let acc = ledger
            .get_account(client(1))
            .expect("client(1) account should exist");
        assert_eq!(acc.available, amount("100"));
    }

    #[test]
    fn test_held_never_negative_invariant() {
        let mut ledger = Ledger::new();
        ledger.process(
            TransactionType::Deposit,
            client(1),
            tx(1),
            Some(amount("100")),
        );
        ledger.process(TransactionType::Dispute, client(1), tx(1), None);
        ledger.process(TransactionType::Resolve, client(1), tx(1), None);
        let acc = ledger
            .get_account(client(1))
            .expect("client(1) account should exist");
        assert!(!acc.held.is_negative());
        assert_eq!(acc.held, amount("0"));
    }
}
