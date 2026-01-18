// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

// Enforce mutual exclusivity of transport features
#[cfg(all(feature = "channel-transport", feature = "udp-transport"))]
compile_error!("Features 'channel-transport' and 'udp-transport' are mutually exclusive. Use --no-default-features to disable 'udp-transport'.");

pub mod async_transport;
pub mod embassy_transport;

#[cfg(feature = "channel-transport")]
pub mod channel;

#[cfg(feature = "channel-transport")]
pub use channel::setup;

#[cfg(feature = "udp-transport")]
pub mod udp;

#[cfg(feature = "udp-transport")]
pub use udp::setup;
