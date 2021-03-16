use bank_payments_system::account_manager::AccountManager;
use bank_payments_system::tx_processor::TxProcessor;
use bank_payments_system::tx_stream_reader::TxStreamReader;
use std::collections::HashMap;
use std::fs::File;
use std::sync::Arc;
use std::sync::RwLock;

#[tokio::test]
async fn payments_system_does_not_panic_against_csv() {
    let file_reader = File::open(String::from("transactions.csv")).expect("Cannot open CSV file!");
    let tx_reader = TxStreamReader::new(file_reader).expect("Cannot create a Tx stream reader!");
    let accounts = Arc::new(RwLock::new(HashMap::new()));
    let transactions = Arc::new(RwLock::new(HashMap::new()));
    let acc_man = AccountManager::new(accounts, transactions);
    let mut tx_processor = TxProcessor::new(tx_reader, acc_man);
    tx_processor.start().await;
    tx_processor.print_accounts();
}
