// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

pub mod async_transport;
pub mod embassy_transport;

#[cfg(feature = "channel-transport")]
pub mod channel;

#[cfg(feature = "channel-transport")]
pub use channel::{ChannelTransport, ChannelTransportHub};

#[cfg(feature = "udp-transport")]
pub mod udp;

#[cfg(feature = "udp-transport")]
pub mod raft_msg_serde;

#[cfg(feature = "udp-transport")]
pub mod net_config;

#[cfg(feature = "udp-transport")]
pub mod net_driver;

pub mod setup;

// Re-exports for convenience (suppress unused warnings - these are for external use)
#[cfg(feature = "udp-transport")]
#[allow(unused_imports)]
pub use udp::UdpTransport;

#[cfg(feature = "udp-transport")]
#[allow(unused_imports)]
pub use raft_msg_serde::{WireLogEntry, WireRaftMsg};
