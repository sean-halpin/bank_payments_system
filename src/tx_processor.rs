use crate::account_manager::AccountManager;
use crate::tx_stream_reader::TxStreamReader;
use crate::Transaction;
use crate::DECIMAL_PRECISION;

pub struct TxProcessor<T>
where
    T: std::io::Read,
{
    tx_stream: TxStreamReader<T>,
    acc_man: AccountManager,
}

impl<T: std::io::Read> TxProcessor<T> {
    pub fn new(tx_stream: TxStreamReader<T>, acc_man: AccountManager) -> Self {
        TxProcessor { tx_stream, acc_man }
    }

    pub async fn start(&mut self) {
        for buf in self.tx_stream.stream.records() {
            if let Ok(tx) = buf {
                if let Ok(mut deserialized_tx) = tx.deserialize::<Transaction>(None) {
                    if let Some(amt) = deserialized_tx.amount {
                        deserialized_tx.amount = Some(amt.round_dp(DECIMAL_PRECISION));
                    }
                    if let Err(e) = self.acc_man.process_tx(&deserialized_tx) {
                        eprintln!("Error processing transaction: {}", e);
                    }
                }
            }
        }
    }

    pub fn print_accounts(&mut self) {
        println!("{}", self.acc_man);
    }
}
