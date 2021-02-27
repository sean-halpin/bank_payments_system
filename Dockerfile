FROM rustlang/rust:nightly-buster

WORKDIR /svc/app
COPY . .

RUN cargo install --path .

ENTRYPOINT ["bank_payments_system"]