FROM rust:1.84 as builder

WORKDIR /usr/src/chaoschain
COPY . .

RUN cargo build --release

FROM rust:1.84-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /usr/src/chaoschain/target/release/chaoschain /app/

EXPOSE 3000

ENTRYPOINT ["/app/chaoschain"]
CMD ["demo", "--validators", "3", "--producers", "3", "--web"]
