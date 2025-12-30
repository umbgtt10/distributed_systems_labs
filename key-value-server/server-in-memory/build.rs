fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create .generated directory if it doesn't exist
    std::fs::create_dir_all(".generated")?;

    tonic_prost_build::configure()
        .out_dir(".generated")
        .compile_protos(&["proto/kvservice.proto"], &["proto"])?;
    Ok(())
}
