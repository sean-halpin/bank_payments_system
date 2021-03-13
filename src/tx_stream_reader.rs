use std::error::Error;
use std::io::BufReader;

pub struct TxStreamReader<T> {
    pub stream: csv::Reader<BufReader<T>>,
}

impl<T> TxStreamReader<T>
where
    T: std::io::Read,
{
    pub fn new(reader: T) -> Result<Self, Box<dyn Error>> {
        let buffered_reader = BufReader::new(reader);
        let tsr: csv::Reader<BufReader<T>> = TxStreamReader::csv_reader(buffered_reader);
        Ok(TxStreamReader { stream: tsr })
    }
    fn csv_reader(reader: T) -> csv::Reader<T> {
        let csv_reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .has_headers(true)
            .delimiter(b',')
            .flexible(true)
            .double_quote(false)
            .from_reader(reader);
        csv_reader
    }
}
