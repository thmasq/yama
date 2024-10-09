FROM rust:1.80 as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -rf src

COPY . .
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y libssl-dev pkg-config ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/yama /usr/local/bin/

ENV APP_PORT=8080
ENV APP_DATABASE_URL=postgres://user:password@db:5432/mydb

EXPOSE 8080
CMD ["yama"]
