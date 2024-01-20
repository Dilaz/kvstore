FROM messense/rust-musl-cross:armv7-musleabihf AS builder
WORKDIR /kvstore
COPY . .
RUN cargo build --release --target armv7-unknown-linux-musleabihf

FROM scratch
COPY --from=builder /kvstore/target/armv7-unknown-linux-musleabihf/release/kvstore /kvstore/kvstore
WORKDIR /kvstore
ENTRYPOINT [ "./kvstore" ]
EXPOSE 3000
