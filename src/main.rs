use bank_payments_system::account_manager::AccountManager;
use bank_payments_system::tx_processor::TxProcessor;
use bank_payments_system::tx_stream_reader::TxStreamReader;
use std::fs::File;

#[tokio::main]
async fn main() {
    let csv_path = std::env::args()
        .nth(1)
        .expect("Expected a CSV filename, run with `cargo run -- transactions.csv`");

    let file_reader = File::open(csv_path).expect("Cannot open CSV file!");
    let tx_reader = TxStreamReader::new(file_reader).expect("Cannot create a Tx stream reader!");
    let acc_man = AccountManager::default();
    let mut tx_processor = TxProcessor::new(tx_reader, acc_man);
    tx_processor.start().await;
    tx_processor.print_accounts();
}
