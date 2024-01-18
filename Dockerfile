FROM messense/rust-musl-cross:x86_64-musl AS builder
WORKDIR /kvstore
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM scratch
COPY --from=builder /kvstore/target/x86_64-unknown-linux-musl/release/kvstore /kvstore/kvstore
WORKDIR /kvstore
ENTRYPOINT [ "./kvstore" ]
EXPOSE 3000
