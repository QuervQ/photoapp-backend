FROM rust:1.88-bookworm

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo "fn main() {}" > src/main.rs
RUN cargo build || true

COPY src ./src

EXPOSE 8080
CMD ["cargo", "run"]
