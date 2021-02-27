FROM rustlang/rust:nightly-buster

WORKDIR /svc/app
COPY . .

RUN cargo install --jobs 2 --path .

CMD ["cargo test"]