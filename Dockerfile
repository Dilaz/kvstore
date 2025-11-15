FROM rust:1.83 AS builder
WORKDIR /kvstore
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --release

FROM gcr.io/distroless/cc-debian12
COPY --from=builder /kvstore/target/release/kvstore /kvstore
ENTRYPOINT [ "./kvstore" ]
EXPOSE 3000 50051
