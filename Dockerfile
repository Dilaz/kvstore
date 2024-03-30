FROM rust:1.76 AS builder
WORKDIR /kvstore
COPY . .
RUN cargo build --release

FROM gcr.io/distroless/cc-debian12
COPY --from=builder /kvstore/target/release/kvstore /kvstore
ENTRYPOINT [ "./kvstore" ]
EXPOSE 3000
