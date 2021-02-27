use crate::ClientAccount;
use crate::Transaction;
use crate::TxType;
use rust_decimal::Decimal;
use std::collections::hash_map::Entry::Occupied;
use std::collections::hash_map::Entry::Vacant;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::io;

pub struct AccountManager {
    pub accounts: HashMap<u16, ClientAccount>,
    transactions: HashMap<u32, Transaction>,
}

impl std::fmt::Display for AccountManager {
    fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
        self.to_csv().unwrap();
        Ok(())
    }
}

impl Default for AccountManager {
    fn default() -> Self {
        AccountManager {
            accounts: HashMap::new(),
            transactions: HashMap::new(),
        }
    }
}

impl AccountManager {
    fn to_csv(&self) -> Result<(), Box<dyn Error>> {
        let mut wtr = csv::Writer::from_writer(io::stdout());
        for acc in self.accounts.values() {
            wtr.serialize(acc).unwrap();
        }
        wtr.flush()?;
        Ok(())
    }
    fn process_deposit(&mut self, tx: &Transaction) -> Result<(), Box<dyn Error>> {
        let amount = match tx.amount {
            Some(a) => a,
            None => return Err("Amount Required".into()),
        };
        match self.transactions.entry(tx.tx) {
            Occupied(_) => return Err("Duplicate Transaction".into()),
            Vacant(e) => {
                e.insert(tx.clone());
            }
        }
        match self.accounts.entry(tx.client) {
            Occupied(mut e) => {
                let account = e.get_mut();
                account.available += amount;
                account.total = account.available - account.held;
            }
            Vacant(e) => {
                let new_account = ClientAccount {
                    available: amount,
                    client: tx.client,
                    held: Decimal::new(0, 0),
                    locked: false,
                    total: amount,
                };
                e.insert(new_account);
            }
        }
        Ok(())
    }

    fn process_withdraw(&mut self, tx: &Transaction) -> Result<(), Box<dyn Error>> {
        let amount = match tx.amount {
            Some(a) => a,
            None => return Err("Amount Required".into()),
        };
        match self.transactions.entry(tx.tx) {
            Occupied(_) => return Err("Duplicate Transaction".into()),
            Vacant(e) => {
                e.insert(tx.clone());
            }
        }
        match self.accounts.entry(tx.client) {
            Occupied(mut e) => {
                let account = e.get_mut();
                if account.available - amount < Decimal::new(0, 0) {
                    return Err("Insufficient Funds".into());
                }
                account.available -= amount;
                account.total = account.available - account.held;
            }
            Vacant(e) => {
                let new_account = ClientAccount {
                    available: -amount,
                    client: tx.client,
                    held: Decimal::new(0, 0),
                    locked: false,
                    total: -amount,
                };
                e.insert(new_account);
            }
        }
        Ok(())
    }

    fn process_dispute(&mut self, tx: &Transaction) -> Result<(), Box<dyn Error>> {
        let mut _account = match self.accounts.entry(tx.client) {
            Occupied(entry) => entry,
            Vacant(_) => {
                return Err("No Associated Client Account Found".into());
            }
        };
        match self.transactions.entry(tx.tx) {
            Occupied(mut e) => {
                let disputed_tx = e.get_mut();
                let account = _account.get_mut();
                if disputed_tx.tx_type.as_ref().unwrap() != &TxType::Deposit {
                    return Err("Only a Deposit can be disputed".into());
                }
                let amount = match disputed_tx.amount {
                    Some(a) => a,
                    None => return Err("Amount Required".into()),
                };
                account.available -= amount;
                account.held += amount;
                disputed_tx.is_disputed = true;
            }
            Vacant(_) => {
                return Err("No Associated Transaction to-be-disputed could be Found".into());
            }
        };
        Ok(())
    }

    fn process_resolve(&mut self, tx: &Transaction) -> Result<(), Box<dyn Error>> {
        let mut _account = match self.accounts.entry(tx.client) {
            Occupied(entry) => entry,
            Vacant(_) => {
                return Err("No Associated Client Account Found".into());
            }
        };
        match self.transactions.entry(tx.tx) {
            Occupied(mut e) => {
                let disputed_tx = e.get_mut();
                if !disputed_tx.is_disputed {
                    return Err("Transaction is not disputed".into());
                }
                let account = _account.get_mut();
                let amount = match disputed_tx.amount {
                    Some(a) => a,
                    None => return Err("Amount Required".into()),
                };
                account.available += amount;
                account.held -= amount;
                disputed_tx.is_disputed = false;
            }
            Vacant(_) => {
                return Err("No Associated Transaction to-be-resolved could be Found".into());
            }
        };
        Ok(())
    }

    fn process_chargeback(&mut self, tx: &Transaction) -> Result<(), Box<dyn Error>> {
        let mut _account = match self.accounts.entry(tx.client) {
            Occupied(entry) => entry,
            Vacant(_) => {
                return Err("No Associated Client Account Found".into());
            }
        };
        match self.transactions.entry(tx.tx) {
            Occupied(mut e) => {
                let disputed_tx = e.get_mut();
                if !disputed_tx.is_disputed {
                    return Err("Transaction is not disputed".into());
                }
                let account = _account.get_mut();
                let amount = match disputed_tx.amount {
                    Some(a) => a,
                    None => return Err("Amount Required".into()),
                };
                account.held -= amount;
                account.total = account.available - account.held;
                account.locked = true;
            }
            Vacant(_) => {
                return Err("No Associated Transaction to-be-chargedback could be Found".into());
            }
        };
        Ok(())
    }

    pub fn process_tx(&mut self, tx: &Transaction) -> Result<(), Box<dyn Error>> {
        match &tx.tx_type {
            Some(t) => match t {
                TxType::Deposit => self.process_deposit(tx)?,
                TxType::Withdraw => self.process_withdraw(tx)?,
                TxType::Dispute => self.process_dispute(tx)?,
                TxType::Resolve => self.process_resolve(tx)?,
                TxType::Chargeback => self.process_chargeback(tx)?,
            },
            None => return Err("No Tx Type provided".into()),
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_new_account() {
        let mut acc_man = AccountManager::default();
        let client_id = 1u16;
        let tx = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(1, 0)),
            is_disputed: false,
        };
        let result = acc_man.process_tx(&tx);
        assert!(result.is_ok());
        let maybe_account = acc_man.accounts.get(&client_id);
        assert!(maybe_account.is_some());
        let account: &ClientAccount = maybe_account.unwrap();
        assert_eq!(account.available, Decimal::new(1, 0));
        assert_eq!(account.client, client_id);
        assert_eq!(account.held, Decimal::new(0, 0));
        assert_eq!(account.locked, false);
        assert_eq!(account.total, Decimal::new(1, 0));
    }

    #[test]
    fn deposit_duplicate_tx() {
        let mut acc_man = AccountManager::default();
        let client_id = 1u16;
        let tx1 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(1, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx1).is_ok());
        let tx2 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(1, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx2).is_err());

        let maybe_account = acc_man.accounts.get(&client_id);
        assert!(maybe_account.is_some());
        let account: &ClientAccount = maybe_account.unwrap();
        assert_eq!(account.available, Decimal::new(1, 0));
        assert_eq!(account.client, client_id);
        assert_eq!(account.held, Decimal::new(0, 0));
        assert_eq!(account.locked, false);
        assert_eq!(account.total, Decimal::new(1, 0));
    }

    #[test]
    fn deposit_multiple_tx() {
        let mut acc_man = AccountManager::default();
        let client_id = 1u16;
        let tx1 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(1, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx1).is_ok());
        let tx2 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 2u32,
            amount: Some(Decimal::new(1, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx2).is_ok());

        let maybe_account = acc_man.accounts.get(&client_id);
        assert!(maybe_account.is_some());
        let account: &ClientAccount = maybe_account.unwrap();
        assert_eq!(account.available, Decimal::new(2, 0));
        assert_eq!(account.client, client_id);
        assert_eq!(account.held, Decimal::new(0, 0));
        assert_eq!(account.locked, false);
        assert_eq!(account.total, Decimal::new(2, 0));
    }

    #[test]
    fn withdraw_new_account() {
        let mut acc_man = AccountManager::default();
        let client_id = 1u16;
        let tx = Transaction {
            tx_type: Some(TxType::Withdraw),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(1, 0)),
            is_disputed: false,
        };
        let result = acc_man.process_tx(&tx);
        assert!(result.is_ok());
        let maybe_account = acc_man.accounts.get(&client_id);
        assert!(maybe_account.is_some());
        let account: &ClientAccount = maybe_account.unwrap();
        assert_eq!(account.available, Decimal::new(-1, 0));
        assert_eq!(account.client, client_id);
        assert_eq!(account.held, Decimal::new(0, 0));
        assert_eq!(account.locked, false);
        assert_eq!(account.total, Decimal::new(-1, 0));
    }

    #[test]
    fn withdraw_duplicate_tx() {
        let mut acc_man = AccountManager::default();
        let client_id = 1u16;
        let tx1 = Transaction {
            tx_type: Some(TxType::Withdraw),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(1, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx1).is_ok());
        let tx2 = Transaction {
            tx_type: Some(TxType::Withdraw),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(1, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx2).is_err());

        let maybe_account = acc_man.accounts.get(&client_id);
        assert!(maybe_account.is_some());
        let account: &ClientAccount = maybe_account.unwrap();
        assert_eq!(account.available, Decimal::new(-1, 0));
        assert_eq!(account.client, client_id);
        assert_eq!(account.held, Decimal::new(0, 0));
        assert_eq!(account.locked, false);
        assert_eq!(account.total, Decimal::new(-1, 0));
    }

    #[test]
    fn withdraw_multiple_tx() {
        let mut acc_man = AccountManager::default();
        let client_id = 1u16;
        let tx1 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(10, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx1).is_ok());
        let tx2 = Transaction {
            tx_type: Some(TxType::Withdraw),
            client: client_id,
            tx: 2u32,
            amount: Some(Decimal::new(1, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx2).is_ok());
        let tx3 = Transaction {
            tx_type: Some(TxType::Withdraw),
            client: client_id,
            tx: 3u32,
            amount: Some(Decimal::new(1, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx3).is_ok());

        let maybe_account = acc_man.accounts.get(&client_id);
        assert!(maybe_account.is_some());
        let account: &ClientAccount = maybe_account.unwrap();
        assert_eq!(account.available, Decimal::new(8, 0));
        assert_eq!(account.client, client_id);
        assert_eq!(account.held, Decimal::new(0, 0));
        assert_eq!(account.locked, false);
        assert_eq!(account.total, Decimal::new(8, 0));
    }

    #[test]
    fn withdraw_insufficient_funds_tx() {
        let mut acc_man = AccountManager::default();
        let client_id = 1u16;
        let tx1 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(10, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx1).is_ok());
        let tx2 = Transaction {
            tx_type: Some(TxType::Withdraw),
            client: client_id,
            tx: 2u32,
            amount: Some(Decimal::new(11, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx2).is_err());

        let maybe_account = acc_man.accounts.get(&client_id);
        assert!(maybe_account.is_some());
        let account: &ClientAccount = maybe_account.unwrap();
        assert_eq!(account.available, Decimal::new(10, 0));
        assert_eq!(account.client, client_id);
        assert_eq!(account.held, Decimal::new(0, 0));
        assert_eq!(account.locked, false);
        assert_eq!(account.total, Decimal::new(10, 0));
    }

    #[test]
    fn dispute_a_deposit_tx() {
        let mut acc_man = AccountManager::default();
        let client_id = 1u16;
        let tx1 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(5, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx1).is_ok());
        let tx2 = Transaction {
            tx_type: Some(TxType::Dispute),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx2).is_ok());

        let maybe_account = acc_man.accounts.get(&client_id);
        assert!(maybe_account.is_some());
        let account: &ClientAccount = maybe_account.unwrap();
        assert_eq!(account.available, Decimal::new(0, 0));
        assert_eq!(account.client, client_id);
        assert_eq!(account.held, Decimal::new(5, 0));
        assert_eq!(account.locked, false);
        assert_eq!(account.total, Decimal::new(5, 0));
        match acc_man.transactions.entry(1u32) {
            Occupied(e) => assert_eq!(e.get().is_disputed, true),
            Vacant(_e) => assert!(false),
        };
    }

    #[test]
    fn dispute_a_withdraw_tx() {
        let mut acc_man = AccountManager::default();
        let client_id = 1u16;
        let tx1 = Transaction {
            tx_type: Some(TxType::Withdraw),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(9, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx1).is_ok());
        let tx2 = Transaction {
            tx_type: Some(TxType::Dispute),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx2).is_err());

        let maybe_account = acc_man.accounts.get(&client_id);
        assert!(maybe_account.is_some());
        let account: &ClientAccount = maybe_account.unwrap();
        assert_eq!(account.available, Decimal::new(-9, 0));
        assert_eq!(account.client, client_id);
        assert_eq!(account.held, Decimal::new(0, 0));
        assert_eq!(account.locked, false);
        assert_eq!(account.total, Decimal::new(-9, 0));
        match acc_man.transactions.entry(1u32) {
            Occupied(e) => assert_eq!(e.get().is_disputed, false),
            Vacant(_e) => assert!(false),
        };
    }

    #[test]
    fn resolve_a_dispute_tx() {
        let mut acc_man = AccountManager::default();
        let client_id = 1u16;
        let tx1 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(9, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx1).is_ok());
        let tx2 = Transaction {
            tx_type: Some(TxType::Dispute),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx2).is_ok());
        let tx3 = Transaction {
            tx_type: Some(TxType::Resolve),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx3).is_ok());
        let maybe_account = acc_man.accounts.get(&client_id);
        assert!(maybe_account.is_some());
        let account: &ClientAccount = maybe_account.unwrap();
        assert_eq!(account.available, Decimal::new(9, 0));
        assert_eq!(account.client, client_id);
        assert_eq!(account.held, Decimal::new(0, 0));
        assert_eq!(account.locked, false);
        assert_eq!(account.total, Decimal::new(9, 0));
        match acc_man.transactions.entry(1u32) {
            Occupied(e) => assert_eq!(e.get().is_disputed, false),
            Vacant(_e) => assert!(false),
        };
    }

    #[test]
    fn resolve_a_non_dispute_tx() {
        let mut acc_man = AccountManager::default();
        let client_id = 1u16;
        let tx1 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(9, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx1).is_ok());
        let tx3 = Transaction {
            tx_type: Some(TxType::Resolve),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx3).is_err());
    }

    #[test]
    fn chargeback_a_dispute_tx() {
        let mut acc_man = AccountManager::default();
        let client_id = 1u16;
        let tx1 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(9, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx1).is_ok());
        let tx2 = Transaction {
            tx_type: Some(TxType::Dispute),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx2).is_ok());
        let tx3 = Transaction {
            tx_type: Some(TxType::Chargeback),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx3).is_ok());
        let maybe_account = acc_man.accounts.get(&client_id);
        assert!(maybe_account.is_some());
        let account: &ClientAccount = maybe_account.unwrap();
        assert_eq!(account.available, Decimal::new(0, 0));
        assert_eq!(account.client, client_id);
        assert_eq!(account.held, Decimal::new(0, 0));
        assert_eq!(account.locked, true);
        assert_eq!(account.total, Decimal::new(0, 0));
        match acc_man.transactions.entry(1u32) {
            Occupied(e) => assert_eq!(e.get().is_disputed, true),
            Vacant(_e) => assert!(false),
        };
    }

    #[test]
    fn chargeback_a_non_dispute_tx() {
        let mut acc_man = AccountManager::default();
        let client_id = 1u16;
        let tx1 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(9, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx1).is_ok());
        let tx3 = Transaction {
            tx_type: Some(TxType::Chargeback),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx3).is_err());
    }

    #[test]
    fn chargeback_a_non_existent_tx() {
        let mut acc_man = AccountManager::default();
        let client_id = 1u16;
        let tx3 = Transaction {
            tx_type: Some(TxType::Chargeback),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx3).is_err());
    }

    #[test]
    fn resolve_a_non_existent_tx() {
        let mut acc_man = AccountManager::default();
        let client_id = 1u16;
        let tx3 = Transaction {
            tx_type: Some(TxType::Resolve),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx3).is_err());
    }

    #[test]
    fn dispute_a_non_existent_tx() {
        let mut acc_man = AccountManager::default();
        let client_id = 1u16;
        let tx3 = Transaction {
            tx_type: Some(TxType::Dispute),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(&tx3).is_err());
    }
}
