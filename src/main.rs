use bank_payments_system::account_manager::AccountManager;
use bank_payments_system::tx_processor::TxProcessor;
use bank_payments_system::tx_stream_reader::TxStreamReader;

#[tokio::main]
async fn main() {
    let csv_path = std::env::args()
        .nth(1)
        .expect("Expected a CSV filename, run with `cargo run -- transactions.csv`");

    let tx_reader = TxStreamReader::new_from_csv(csv_path).unwrap();
    let acc_man = AccountManager::new();
    let mut tx_processor = TxProcessor::new(tx_reader, acc_man);
    tx_processor.start().await;
    tx_processor.print_accounts();
}
