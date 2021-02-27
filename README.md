### Overview

A payments system to manage bank accounts with `deposit`, `withdraw`, `dispute`, `resolve` & `chargeback` type transactions

### Build Status

[![Build Status](https://travis-ci.com/sean-halpin/bank_payments_system.svg?branch=master)](https://travis-ci.com/sean-halpin/bank_payments_system)

The service is being built, linted & tested automatically on each commit with [travis-ci](https://travis-ci.com/github/sean-halpin/bank_payments_system)

### Coverage 

[![codecov](https://codecov.io/gh/sean-halpin/bank_payments_system/branch/master/graph/badge.svg?token=yxIQNIUAGJ)](https://codecov.io/gh/sean-halpin/bank_payments_system)

A test coverage report is being generated on the build pipeline with `grcov` and results of each build can be seen on [codecov.io](https://codecov.io/gh/sean-halpin/bank_payments_system)


### Service Layout

```
src
├── account_manager.rs
├── lib.rs
├── main.rs
├── tx_processor.rs
└── tx_stream_reader.rs
```

The `account_manager.rs` file contains the logic for processing transaction types.
Tests for the logic of those transactions are included in that file. 

The `tx_processor.rs` contains the logic for reading transactions and pushing them to the account manager. 

The `tx_stream_reader.rs` is reading lines & deserializing into `Transaction` structs. This gives us a mechanism to process a stream of transactions one by one & avoid loading the whole CSV into memory.

```
tests
└── integration_test.rs
```

A simple integration style test is run from `integration_test.rs`.  
This test is ingesting the `transactions.csv` in the project's root directory & will output a csv to stdout showing the account's latest status

```
transactions.csv
```

Some test data manually created to run against the application & integration test. 

### Build

```
$ cargo build
```

### Test 

```
$ cargo test
```

### Linting

```
cargo clippy
```

### Run 

```
$ cargo run -- transactions.csv
```

### Capture Output

Piping stdout to a file will yield a csv showing account status after transaction processing. 
```
$ cargo run -- transactions.csv > out.csv
$ cat out.csv
client,available,held,total,locked
1,1.5,0,1.5,false
2,2,0,2,false
4,2.3422,0.0000,2.3422,true
3,21.5578,0,21.5578,false
```
