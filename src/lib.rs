use rust_decimal::Decimal;

pub mod account_manager;
pub mod tx_processor;
pub mod tx_stream_reader;

#[macro_use]
extern crate serde_derive;

#[derive(Debug, Serialize)]
pub struct ClientAccount {
    client: u16,
    available: Decimal,
    held: Decimal,
    total: Decimal,
    locked: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Transaction {
    #[serde(default)]
    r#type: String,
    #[serde(default)]
    client: u16,
    #[serde(default)]
    tx: u32,
    #[serde(default)]
    amount: Option<Decimal>,
    #[serde(default)]
    is_disputed: bool,
}
