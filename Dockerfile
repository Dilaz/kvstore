FROM rustlang/rust:nightly AS builder
WORKDIR /kvstore
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY kvstore-client/Cargo.toml ./kvstore-client/
COPY fuzz/Cargo.toml ./fuzz/

# Copy build scripts
COPY build.rs ./
COPY kvstore-client/build.rs ./kvstore-client/

# Copy proto files (needed for build.rs)
COPY proto ./proto

# Create dummy source files to build only dependencies
RUN mkdir -p src && echo "fn main() {}" > src/main.rs && echo "pub fn dummy() {}" > src/lib.rs
RUN mkdir -p kvstore-client/src && touch kvstore-client/src/lib.rs
RUN mkdir -p fuzz/fuzz_targets && echo "fn main() {}" > fuzz/fuzz_targets/store_fuzz.rs
RUN mkdir -p benches && touch benches/benchmarks.rs

# Build dependencies
RUN cargo build --release --bin kvstore

# Remove dummy source files
RUN rm -rf src kvstore-client/src fuzz/fuzz_targets benches

# Copy full source and do final build
COPY . .
RUN cargo build --release --bin kvstore

FROM gcr.io/distroless/cc-debian12
COPY --from=builder /kvstore/target/release/kvstore /kvstore
ENTRYPOINT [ "./kvstore" ]
EXPOSE 3000 50051
