use bank_payments_system::account_manager::AccountManager;
use bank_payments_system::tx_processor::TxProcessor;
use bank_payments_system::tx_stream_reader::TxStreamReader;
use std::fs::File;

#[tokio::test]
async fn payments_system_does_not_panic_against_csv() {
    let file_reader = File::open(String::from("transactions.csv")).expect("Cannot open CSV file!");
    let tx_reader = TxStreamReader::new(file_reader).expect("Cannot create a Tx stream reader!");
    let acc_man = AccountManager::default();
    let mut tx_processor = TxProcessor::new(tx_reader, acc_man);
    tx_processor.start().await;
    tx_processor.print_accounts();
}
