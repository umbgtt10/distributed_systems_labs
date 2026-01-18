Write-Host "Running Embassy Simulation with Channel Transport (In-Memory)..."
# We must disable default features because udp-transport is now the default
cargo run --release --no-default-features --features channel-transport
