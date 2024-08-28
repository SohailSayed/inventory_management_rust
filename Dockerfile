FROM rust:latest

COPY ./ ./

RUN cargo build --release

CMD ["./target/release/inventory_management_rust"]
