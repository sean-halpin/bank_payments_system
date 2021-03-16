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
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use threadpool::ThreadPool;

pub struct AccountManager {
    pub accounts: Arc<RwLock<HashMap<u16, Mutex<ClientAccount>>>>,
    transactions: Arc<RwLock<HashMap<u32, Mutex<Transaction>>>>,
    pool: ThreadPool,
}

impl std::fmt::Display for AccountManager {
    fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
        self.to_csv().unwrap();
        Ok(())
    }
}

impl AccountManager {
    pub fn new(
        accounts: Arc<RwLock<HashMap<u16, Mutex<ClientAccount>>>>,
        transactions: Arc<RwLock<HashMap<u32, Mutex<Transaction>>>>,
    ) -> Self {
        let pool = ThreadPool::new(2);
        AccountManager {
            accounts,
            transactions,
            pool,
        }
    }

    fn to_csv(&self) -> Result<(), Box<dyn Error>> {
        let mut wtr = csv::Writer::from_writer(io::stdout());
        for acc in self.accounts.read().expect("RwLock poisoned").values() {
            wtr.serialize(acc)?;
        }
        wtr.flush()?;
        Ok(())
    }
    fn process_deposit(
        accounts: Arc<RwLock<HashMap<u16, Mutex<ClientAccount>>>>,
        transactions: Arc<RwLock<HashMap<u32, Mutex<Transaction>>>>,
        tx: &Transaction,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let amount = match tx.amount {
            Some(a) => {
                if a.lt(&Decimal::new(0, 0)) {
                    return Err("Cannot Deposit a Negative Amount".into());
                } else {
                    a
                }
            }
            None => return Err("Amount Required".into()),
        };
        match transactions.write().expect("Unexpected Lock").entry(tx.tx) {
            Occupied(_) => return Err("Duplicate Transaction".into()),
            Vacant(e) => {
                e.insert(Mutex::new(tx.clone()));
            }
        }
        match accounts.write().expect("Unexpected Lock").entry(tx.client) {
            Occupied(mut e) => {
                let mut account = e.get_mut().get_mut().unwrap();
                if account.locked {
                    return Err("Account Locked due to Chargeback".into());
                }
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
                e.insert(Mutex::new(new_account));
            }
        }
        Ok(())
    }

    fn process_withdraw(
        accounts: Arc<RwLock<HashMap<u16, Mutex<ClientAccount>>>>,
        transactions: Arc<RwLock<HashMap<u32, Mutex<Transaction>>>>,
        tx: &Transaction,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let amount = match tx.amount {
            Some(a) => {
                if a.lt(&Decimal::new(0, 0)) {
                    return Err("Cannot Withdraw a Negative Amount".into());
                } else {
                    a
                }
            }
            None => return Err("Amount Required".into()),
        };
        match transactions.write().expect("Unexpected Lock").entry(tx.tx) {
            Occupied(_) => return Err("Duplicate Transaction".into()),
            Vacant(e) => {
                e.insert(Mutex::new(tx.clone()));
            }
        }
        match accounts.write().expect("Unexpected Lock").entry(tx.client) {
            Occupied(mut e) => {
                let mut account = e.get_mut().get_mut().unwrap();
                if account.locked {
                    return Err("Account Locked due to Chargeback".into());
                }
                if (account.available - amount).lt(&Decimal::new(0, 0)) {
                    return Err("Insufficient Funds".into());
                }
                account.available -= amount;
                account.total = account.available - account.held;
            }
            Vacant(_) => return Err("Cannot withdraw from a non existent account".into()),
        }
        Ok(())
    }

    fn process_dispute(
        accounts: Arc<RwLock<HashMap<u16, Mutex<ClientAccount>>>>,
        transactions: Arc<RwLock<HashMap<u32, Mutex<Transaction>>>>,
        tx: &Transaction,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut accounts = accounts.write().expect("Unexpected Lock");
        let mut _account = match accounts.entry(tx.client) {
            Occupied(entry) => {
                if entry.get().lock().unwrap().locked {
                    return Err("Account Locked due to Chargeback".into());
                }
                entry
            }
            Vacant(_) => {
                return Err("No Associated Client Account Found".into());
            }
        };
        match transactions.write().expect("Unexpected Lock").entry(tx.tx) {
            Occupied(mut e) => {
                let mut disputed_tx = e.get_mut().get_mut().unwrap();
                let account = _account.get_mut().get_mut().unwrap();
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

    fn process_resolve(
        accounts: Arc<RwLock<HashMap<u16, Mutex<ClientAccount>>>>,
        transactions: Arc<RwLock<HashMap<u32, Mutex<Transaction>>>>,
        tx: &Transaction,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut accounts = accounts.write().expect("Unexpected Lock");
        let mut _account = match accounts.entry(tx.client) {
            Occupied(entry) => {
                if entry.get().lock().unwrap().locked {
                    return Err("Account Locked due to Chargeback".into());
                }
                entry
            }
            Vacant(_) => {
                return Err("No Associated Client Account Found".into());
            }
        };
        match transactions.write().expect("Unexpected Lock").entry(tx.tx) {
            Occupied(mut e) => {
                let mut disputed_tx = e.get_mut().get_mut().unwrap();
                if !disputed_tx.is_disputed {
                    return Err("Transaction is not disputed".into());
                }
                let account = _account.get_mut().get_mut().unwrap();
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

    fn process_chargeback(
        accounts: Arc<RwLock<HashMap<u16, Mutex<ClientAccount>>>>,
        transactions: Arc<RwLock<HashMap<u32, Mutex<Transaction>>>>,
        tx: &Transaction,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut accounts = accounts.write().expect("Unexpected Lock");
        let mut _account = match accounts.entry(tx.client) {
            Occupied(entry) => {
                if entry.get().lock().unwrap().locked {
                    return Err("Account Locked due to Chargeback".into());
                }
                entry
            }
            Vacant(_) => {
                return Err("No Associated Client Account Found".into());
            }
        };
        match transactions.write().expect("Unexpected Lock").entry(tx.tx) {
            Occupied(mut e) => {
                let disputed_tx = e.get_mut().get_mut().unwrap();
                if !disputed_tx.is_disputed {
                    return Err("Transaction is not disputed".into());
                }
                let mut account = _account.get_mut().get_mut().unwrap();
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

    pub fn process_tx(&mut self, tx: Transaction) -> Result<(), Box<dyn Error>> {
        let accs = self.accounts.clone();
        let txs = self.transactions.clone();
        let (txc, rxc) = channel();
        self.pool.execute(move || {
            let result = match &tx.tx_type {
                Some(t) => match t {
                    TxType::Deposit => txc.send(Self::process_deposit(accs, txs, &tx)),
                    TxType::Withdraw => txc.send(Self::process_withdraw(accs, txs, &tx)),
                    TxType::Dispute => txc.send(Self::process_dispute(accs, txs, &tx)),
                    TxType::Resolve => txc.send(Self::process_resolve(accs, txs, &tx)),
                    TxType::Chargeback => txc.send(Self::process_chargeback(accs, txs, &tx)),
                },
                None => txc.send(Err("No Tx Type provided".into())),
            };
            if let Ok(_r) = result {
                println!("Tx Processed");
            } else {
                eprintln!("Tx Not Processed");
            }
        });
        if let Ok(thread_result) = rxc.recv() {
            println!("Tx Thread Finished");
            if let Err(tx_process_result) = thread_result {
                return Err(tx_process_result);
            }
            Ok(())
        } else {
            eprintln!("Tx Thread Failed");
            Err("Error".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_new_account() {
        let mut acc_man = AccountManager::new(
            Arc::new(RwLock::new(HashMap::new())),
            Arc::new(RwLock::new(HashMap::new())),
        );
        let client_id = 1u16;
        let tx = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(1, 0)),
            is_disputed: false,
        };
        let result = acc_man.process_tx(tx);
        assert!(result.is_ok());
        let acc_man_lock = acc_man.accounts.write().expect("Unexpected Lock");
        let maybe_account = acc_man_lock.get(&client_id);
        assert!(maybe_account.is_some());
        let account: &ClientAccount = &maybe_account.unwrap().lock().unwrap();
        assert_eq!(account.available, Decimal::new(1, 0));
        assert_eq!(account.client, client_id);
        assert_eq!(account.held, Decimal::new(0, 0));
        assert_eq!(account.locked, false);
        assert_eq!(account.total, Decimal::new(1, 0));
    }

    #[test]
    fn deposit_negative_amount_account() {
        let mut acc_man = AccountManager::new(
            Arc::new(RwLock::new(HashMap::new())),
            Arc::new(RwLock::new(HashMap::new())),
        );
        let client_id = 1u16;
        let tx = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(-1, 0)),
            is_disputed: false,
        };
        let result = acc_man.process_tx(tx);
        assert!(result.is_err());
        let acc_man_lock = acc_man.accounts.write().expect("Unexpected Lock");
        let maybe_account = acc_man_lock.get(&client_id);
        assert!(maybe_account.is_none());
    }

    #[test]
    fn withdraw_negative_amount_account() {
        let mut acc_man = AccountManager::new(
            Arc::new(RwLock::new(HashMap::new())),
            Arc::new(RwLock::new(HashMap::new())),
        );
        let client_id = 1u16;
        let tx = Transaction {
            tx_type: Some(TxType::Withdraw),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(-1, 0)),
            is_disputed: false,
        };
        let result = acc_man.process_tx(tx);
        assert!(result.is_err());
        let acc_man_lock = acc_man.accounts.write().expect("Unexpected Lock");
        let maybe_account = acc_man_lock.get(&client_id);
        assert!(maybe_account.is_none());
    }

    #[test]
    fn deposit_duplicate_tx() {
        let mut acc_man = AccountManager::new(
            Arc::new(RwLock::new(HashMap::new())),
            Arc::new(RwLock::new(HashMap::new())),
        );
        let client_id = 1u16;
        let tx1 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(1, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx1).is_ok());
        let tx2 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(1, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx2).is_err());

        let acc_man_lock = acc_man.accounts.write().expect("Unexpected Lock");
        let maybe_account = acc_man_lock.get(&client_id);
        assert!(maybe_account.is_some());
        let account: &ClientAccount = &maybe_account.unwrap().lock().unwrap();
        assert_eq!(account.available, Decimal::new(1, 0));
        assert_eq!(account.client, client_id);
        assert_eq!(account.held, Decimal::new(0, 0));
        assert_eq!(account.locked, false);
        assert_eq!(account.total, Decimal::new(1, 0));
    }

    #[test]
    fn deposit_multiple_tx() {
        let mut acc_man = AccountManager::new(
            Arc::new(RwLock::new(HashMap::new())),
            Arc::new(RwLock::new(HashMap::new())),
        );
        let client_id = 1u16;
        let tx1 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(1, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx1).is_ok());
        let tx2 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 2u32,
            amount: Some(Decimal::new(1, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx2).is_ok());

        let acc_man_lock = acc_man.accounts.write().expect("Unexpected Lock");
        let maybe_account = acc_man_lock.get(&client_id);
        assert!(maybe_account.is_some());
        let account: &ClientAccount = &maybe_account.unwrap().lock().unwrap();
        assert_eq!(account.available, Decimal::new(2, 0));
        assert_eq!(account.client, client_id);
        assert_eq!(account.held, Decimal::new(0, 0));
        assert_eq!(account.locked, false);
        assert_eq!(account.total, Decimal::new(2, 0));
    }

    #[test]
    fn withdraw_new_account() {
        let mut acc_man = AccountManager::new(
            Arc::new(RwLock::new(HashMap::new())),
            Arc::new(RwLock::new(HashMap::new())),
        );
        let client_id = 1u16;
        let tx = Transaction {
            tx_type: Some(TxType::Withdraw),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(1, 0)),
            is_disputed: false,
        };
        let result = acc_man.process_tx(tx);
        assert!(result.is_err());
        let acc_man_lock = acc_man.accounts.write().expect("Unexpected Lock");
        let maybe_account = acc_man_lock.get(&client_id);
        assert!(maybe_account.is_none());
    }

    #[test]
    fn withdraw_duplicate_tx() {
        let mut acc_man = AccountManager::new(
            Arc::new(RwLock::new(HashMap::new())),
            Arc::new(RwLock::new(HashMap::new())),
        );
        let client_id = 1u16;
        let tx = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(9, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx).is_ok());
        let tx1 = Transaction {
            tx_type: Some(TxType::Withdraw),
            client: client_id,
            tx: 2u32,
            amount: Some(Decimal::new(1, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx1).is_ok());
        let tx2 = Transaction {
            tx_type: Some(TxType::Withdraw),
            client: client_id,
            tx: 2u32,
            amount: Some(Decimal::new(1, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx2).is_err());

        let acc_man_lock = acc_man.accounts.write().expect("Unexpected Lock");
        let maybe_account = acc_man_lock.get(&client_id);
        assert!(maybe_account.is_some());
        let account: &ClientAccount = &maybe_account.unwrap().lock().unwrap();
        assert_eq!(account.available, Decimal::new(8, 0));
        assert_eq!(account.client, client_id);
        assert_eq!(account.held, Decimal::new(0, 0));
        assert_eq!(account.locked, false);
        assert_eq!(account.total, Decimal::new(8, 0));
    }

    #[test]
    fn withdraw_multiple_tx() {
        let mut acc_man = AccountManager::new(
            Arc::new(RwLock::new(HashMap::new())),
            Arc::new(RwLock::new(HashMap::new())),
        );
        let client_id = 1u16;
        let tx1 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(10, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx1).is_ok());
        let tx2 = Transaction {
            tx_type: Some(TxType::Withdraw),
            client: client_id,
            tx: 2u32,
            amount: Some(Decimal::new(1, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx2).is_ok());
        let tx3 = Transaction {
            tx_type: Some(TxType::Withdraw),
            client: client_id,
            tx: 3u32,
            amount: Some(Decimal::new(1, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx3).is_ok());

        let acc_man_lock = acc_man.accounts.write().expect("Unexpected Lock");
        let maybe_account = acc_man_lock.get(&client_id);
        assert!(maybe_account.is_some());
        let account: &ClientAccount = &maybe_account.unwrap().lock().unwrap();
        assert_eq!(account.available, Decimal::new(8, 0));
        assert_eq!(account.client, client_id);
        assert_eq!(account.held, Decimal::new(0, 0));
        assert_eq!(account.locked, false);
        assert_eq!(account.total, Decimal::new(8, 0));
    }

    #[test]
    fn withdraw_insufficient_funds_tx() {
        let mut acc_man = AccountManager::new(
            Arc::new(RwLock::new(HashMap::new())),
            Arc::new(RwLock::new(HashMap::new())),
        );
        let client_id = 1u16;
        let tx1 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(10, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx1).is_ok());
        let tx2 = Transaction {
            tx_type: Some(TxType::Withdraw),
            client: client_id,
            tx: 2u32,
            amount: Some(Decimal::new(11, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx2).is_err());

        let acc_man_lock = acc_man.accounts.write().expect("Unexpected Lock");
        let maybe_account = acc_man_lock.get(&client_id);
        assert!(maybe_account.is_some());
        let account: &ClientAccount = &maybe_account.unwrap().lock().unwrap();
        assert_eq!(account.available, Decimal::new(10, 0));
        assert_eq!(account.client, client_id);
        assert_eq!(account.held, Decimal::new(0, 0));
        assert_eq!(account.locked, false);
        assert_eq!(account.total, Decimal::new(10, 0));
    }

    #[test]
    fn dispute_a_deposit_tx() {
        let mut acc_man = AccountManager::new(
            Arc::new(RwLock::new(HashMap::new())),
            Arc::new(RwLock::new(HashMap::new())),
        );
        let client_id = 1u16;
        let tx1 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(5, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx1).is_ok());
        let tx2 = Transaction {
            tx_type: Some(TxType::Dispute),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx2).is_ok());

        let acc_man_lock = acc_man.accounts.write().expect("Unexpected Lock");
        let maybe_account = acc_man_lock.get(&client_id);
        assert!(maybe_account.is_some());
        let account: &ClientAccount = &maybe_account.unwrap().lock().unwrap();
        assert_eq!(account.available, Decimal::new(0, 0));
        assert_eq!(account.client, client_id);
        assert_eq!(account.held, Decimal::new(5, 0));
        assert_eq!(account.locked, false);
        assert_eq!(account.total, Decimal::new(5, 0));
        match acc_man
            .transactions
            .write()
            .expect("Unexpected Lock")
            .entry(1u32)
        {
            Occupied(e) => assert_eq!(e.get().lock().unwrap().is_disputed, true),
            Vacant(_e) => assert!(false),
        };
    }

    #[test]
    fn dispute_a_withdraw_tx() {
        let mut acc_man = AccountManager::new(
            Arc::new(RwLock::new(HashMap::new())),
            Arc::new(RwLock::new(HashMap::new())),
        );
        let client_id = 1u16;
        let tx = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(10, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx).is_ok());
        let tx1 = Transaction {
            tx_type: Some(TxType::Withdraw),
            client: client_id,
            tx: 2u32,
            amount: Some(Decimal::new(9, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx1).is_ok());
        let tx2 = Transaction {
            tx_type: Some(TxType::Dispute),
            client: client_id,
            tx: 2u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx2).is_err());

        let acc_man_lock = acc_man.accounts.write().expect("Unexpected Lock");
        let maybe_account = acc_man_lock.get(&client_id);
        assert!(maybe_account.is_some());
        let account: &ClientAccount = &maybe_account.unwrap().lock().unwrap();
        assert_eq!(account.available, Decimal::new(1, 0));
        assert_eq!(account.client, client_id);
        assert_eq!(account.held, Decimal::new(0, 0));
        assert_eq!(account.locked, false);
        assert_eq!(account.total, Decimal::new(1, 0));
        match acc_man
            .transactions
            .write()
            .expect("Unexpected Lock")
            .entry(1u32)
        {
            Occupied(e) => assert_eq!(e.get().lock().unwrap().is_disputed, false),
            Vacant(_e) => assert!(false),
        };
    }

    #[test]
    fn resolve_a_dispute_tx() {
        let mut acc_man = AccountManager::new(
            Arc::new(RwLock::new(HashMap::new())),
            Arc::new(RwLock::new(HashMap::new())),
        );
        let client_id = 1u16;
        let tx1 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(9, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx1).is_ok());
        let tx2 = Transaction {
            tx_type: Some(TxType::Dispute),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx2).is_ok());
        let tx3 = Transaction {
            tx_type: Some(TxType::Resolve),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx3).is_ok());
        let acc_man_lock = acc_man.accounts.write().expect("Unexpected Lock");
        let maybe_account = acc_man_lock.get(&client_id);
        assert!(maybe_account.is_some());
        let account: &ClientAccount = &maybe_account.unwrap().lock().unwrap();
        assert_eq!(account.available, Decimal::new(9, 0));
        assert_eq!(account.client, client_id);
        assert_eq!(account.held, Decimal::new(0, 0));
        assert_eq!(account.locked, false);
        assert_eq!(account.total, Decimal::new(9, 0));
        match acc_man
            .transactions
            .write()
            .expect("Unexpected Lock")
            .entry(1u32)
        {
            Occupied(e) => assert_eq!(e.get().lock().unwrap().is_disputed, false),
            Vacant(_e) => assert!(false),
        };
    }

    #[test]
    fn resolve_a_non_dispute_tx() {
        let mut acc_man = AccountManager::new(
            Arc::new(RwLock::new(HashMap::new())),
            Arc::new(RwLock::new(HashMap::new())),
        );
        let client_id = 1u16;
        let tx1 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(9, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx1).is_ok());
        let tx3 = Transaction {
            tx_type: Some(TxType::Resolve),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx3).is_err());
    }

    #[test]
    fn chargeback_a_dispute_tx() {
        let mut acc_man = AccountManager::new(
            Arc::new(RwLock::new(HashMap::new())),
            Arc::new(RwLock::new(HashMap::new())),
        );
        let client_id = 1u16;
        let tx1 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(9, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx1).is_ok());
        let tx2 = Transaction {
            tx_type: Some(TxType::Dispute),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx2).is_ok());
        let tx3 = Transaction {
            tx_type: Some(TxType::Chargeback),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx3).is_ok());
        let acc_man_lock = acc_man.accounts.write().expect("Unexpected Lock");
        let maybe_account = acc_man_lock.get(&client_id);
        assert!(maybe_account.is_some());
        let account: &ClientAccount = &maybe_account.unwrap().lock().unwrap();
        assert_eq!(account.available, Decimal::new(0, 0));
        assert_eq!(account.client, client_id);
        assert_eq!(account.held, Decimal::new(0, 0));
        assert_eq!(account.locked, true);
        assert_eq!(account.total, Decimal::new(0, 0));
        match acc_man
            .transactions
            .write()
            .expect("Unexpected Lock")
            .entry(1u32)
        {
            Occupied(e) => assert_eq!(e.get().lock().unwrap().is_disputed, true),
            Vacant(_e) => assert!(false),
        };
    }

    #[test]
    fn cant_deposit_to_a_locked_account() {
        let mut acc_man = AccountManager::new(
            Arc::new(RwLock::new(HashMap::new())),
            Arc::new(RwLock::new(HashMap::new())),
        );
        let client_id = 1u16;
        let tx1 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(9, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx1).is_ok());
        let tx2 = Transaction {
            tx_type: Some(TxType::Dispute),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx2).is_ok());
        let tx3 = Transaction {
            tx_type: Some(TxType::Chargeback),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx3).is_ok());
        let tx4 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(9, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx4).is_err());
        let acc_man_lock = acc_man.accounts.write().expect("Unexpected Lock");
        let maybe_account = acc_man_lock.get(&client_id);
        assert!(maybe_account.is_some());
        let account: &ClientAccount = &maybe_account.unwrap().lock().unwrap();
        assert_eq!(account.available, Decimal::new(0, 0));
        assert_eq!(account.client, client_id);
        assert_eq!(account.held, Decimal::new(0, 0));
        assert_eq!(account.locked, true);
        assert_eq!(account.total, Decimal::new(0, 0));
        match acc_man
            .transactions
            .write()
            .expect("Unexpected Lock")
            .entry(1u32)
        {
            Occupied(e) => assert_eq!(e.get().lock().unwrap().is_disputed, true),
            Vacant(_e) => assert!(false),
        };
    }

    #[test]
    fn chargeback_a_non_dispute_tx() {
        let mut acc_man = AccountManager::new(
            Arc::new(RwLock::new(HashMap::new())),
            Arc::new(RwLock::new(HashMap::new())),
        );
        let client_id = 1u16;
        let tx1 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(9, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx1).is_ok());
        let tx3 = Transaction {
            tx_type: Some(TxType::Chargeback),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx3).is_err());
    }

    #[test]
    fn chargeback_a_non_existent_tx() {
        let mut acc_man = AccountManager::new(
            Arc::new(RwLock::new(HashMap::new())),
            Arc::new(RwLock::new(HashMap::new())),
        );
        let client_id = 1u16;
        let tx1 = Transaction {
            tx_type: Some(TxType::Deposit),
            client: client_id,
            tx: 1u32,
            amount: Some(Decimal::new(9, 0)),
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx1).is_ok());
        let tx3 = Transaction {
            tx_type: Some(TxType::Chargeback),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx3).is_err());
    }

    #[test]
    fn chargeback_a_non_existent_customer() {
        let mut acc_man = AccountManager::new(
            Arc::new(RwLock::new(HashMap::new())),
            Arc::new(RwLock::new(HashMap::new())),
        );
        let client_id = 1u16;
        let tx3 = Transaction {
            tx_type: Some(TxType::Chargeback),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx3).is_err());
    }

    #[test]
    fn resolve_a_non_existent_tx() {
        let mut acc_man = AccountManager::new(
            Arc::new(RwLock::new(HashMap::new())),
            Arc::new(RwLock::new(HashMap::new())),
        );
        let client_id = 1u16;
        let tx3 = Transaction {
            tx_type: Some(TxType::Resolve),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx3).is_err());
    }

    #[test]
    fn dispute_a_non_existent_tx() {
        let mut acc_man = AccountManager::new(
            Arc::new(RwLock::new(HashMap::new())),
            Arc::new(RwLock::new(HashMap::new())),
        );
        let client_id = 1u16;
        let tx3 = Transaction {
            tx_type: Some(TxType::Dispute),
            client: client_id,
            tx: 1u32,
            amount: None,
            is_disputed: false,
        };
        assert!(acc_man.process_tx(tx3).is_err());
    }
}
