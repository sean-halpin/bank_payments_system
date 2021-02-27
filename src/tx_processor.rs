use crate::account_manager::AccountManager;
use crate::tx_stream_reader::TxStreamReader;
use crate::Transaction;

pub struct TxProcessor {
    tx_stream: TxStreamReader,
    acc_man: AccountManager,
}

impl TxProcessor {
    pub fn new(tx_stream: TxStreamReader, acc_man: AccountManager) -> Self {
        TxProcessor { tx_stream, acc_man }
    }

    pub async fn start(&mut self) {
        for buf in self.tx_stream.stream.records() {
            match buf {
                Ok(tx) => {
                    match tx.deserialize::<Transaction>(None) {
                        Ok(d) => {
                            self.acc_man.process_tx(&d).unwrap();
                        }
                        Err(e) => eprintln!("Could not deserialize: {}", e),
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
