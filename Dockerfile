FROM rust:latest

RUN USER=root cargo new --bin inventory_management_rust
WORKDIR /inventory_management_rust

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

RUN cargo build --release
RUN rm src/*.rs

COPY ./src ./src

RUN cargo test --release

RUN rm ./target/release/deps/inventory_management_rust*
RUN cargo install --path .

CMD ["inventory_management_rust"]
