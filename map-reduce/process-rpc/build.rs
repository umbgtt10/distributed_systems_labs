// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create .generated directory if it doesn't exist
    std::fs::create_dir_all(".generated")?;

    tonic_prost_build::configure()
        .out_dir(".generated")
        .compile_protos(&["proto/mapreduce.proto"], &["proto"])?;
    Ok(())
}

