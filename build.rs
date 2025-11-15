fn main() -> Result<(), Box<dyn std::error::Error>> {
    let descriptor_path =
        std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("kvstore_descriptor.bin");

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(&descriptor_path)
        .protoc_arg("--experimental_allow_proto3_optional")
        .compile_protos(&["proto/kvstore.proto"], &["proto"])?;
    Ok(())
}
