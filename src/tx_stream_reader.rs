use std::error::Error;
use std::fs::File;
use std::io::BufReader;

pub struct TxStreamReader {
    pub stream: csv::Reader<BufReader<File>>,
}

impl TxStreamReader {
    pub fn new_from_csv(csv_path: String) -> Result<Self, Box<dyn Error>> {
        let file = File::open(csv_path)?;
        let buffered_file_reader = BufReader::new(file);
        let tsr: csv::Reader<BufReader<File>> = TxStreamReader::csv_reader(buffered_file_reader)?;
        Ok(TxStreamReader { stream: tsr })
    }
    fn csv_reader(reader: BufReader<File>) -> Result<csv::Reader<BufReader<File>>, Box<dyn Error>> {
        let csv_reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .has_headers(true)
            .delimiter(b',')
            .flexible(true)
            .double_quote(false)
            .from_reader(reader);
        Ok(csv_reader)
    }
}
