use crate::account_manager::AccountManager;
use crate::tx_stream_reader::TxStreamReader;
use crate::Transaction;
use crate::DECIMAL_PRECISION;

pub struct TxProcessor<T> where T: std::io::Read {
    tx_stream: TxStreamReader<T>,
    acc_man: AccountManager,
}

impl<T: std::io::Read> TxProcessor<T> {
    pub fn new(tx_stream: TxStreamReader<T>, acc_man: AccountManager) -> Self {
        TxProcessor { tx_stream, acc_man }
    }

    pub async fn start(&mut self) {
        for buf in self.tx_stream.stream.records() {
            match buf {
                Ok(tx) => {
                    match tx.deserialize::<Transaction>(None) {
                        Ok(mut deserialized_tx) => {
                            deserialized_tx.amount = match deserialized_tx.amount {
                                Some(a) => Some(a.round_dp(DECIMAL_PRECISION)),
                                None => None,
                            };
                            match self.acc_man.process_tx(&deserialized_tx) {
                                Ok(_) => {}
                                Err(e) => eprintln!("Error: {} : {:?}", e, tx),
                            };
                        }
                        Err(e) => eprintln!("Error: {} : {:?}", e, tx),
                    };
                }
                Err(e) => eprintln!("Could not read line: {}", e),
            }
        }
    }

    pub fn print_accounts(&mut self) {
        println!("{}", self.acc_man);
    }
}
