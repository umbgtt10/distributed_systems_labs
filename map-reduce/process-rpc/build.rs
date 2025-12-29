fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .out_dir(".generated")
        .compile_protos(&["proto/mapreduce.proto"], &["proto"])?;
    Ok(())
}
