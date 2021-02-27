### Overview

A payments system to manage bank accounts with `deposit`, `withdraw`, `dispute`, `resolve` & `chargeback` type transactions

### Build Status

[![Build Status](https://travis-ci.com/sean-halpin/bank_payments_system.svg?branch=master)](https://travis-ci.com/sean-halpin/bank_payments_system)

### Coverage 

[![codecov](https://codecov.io/gh/sean-halpin/bank_payments_system/branch/master/graph/badge.svg?token=yxIQNIUAGJ)](https://codecov.io/gh/sean-halpin/bank_payments_system)

### Build

```
$ cargo build
```

### Test 

```
$ cargo test
```

### Run 

```
$ cargo run -- transactions.csv
```

### Capture Output

Piping stdout to a file will yield a csv showing accounts. 
```
$ cargo run -- transactions.csv > out.csv
$ cat out.csv
client,available,held,total,locked
2,-1,0,-1,false
1,1.5,0,1.5,false
4,2.3422,0.0000,2.3422,true
3,21.5578,0,21.5578,false
```
