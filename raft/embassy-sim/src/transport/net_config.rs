// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Network configuration for UDP transport
//! Static buffers and configuration for embassy-net Stack

use core::mem::MaybeUninit;
use embassy_net::{Config, Ipv4Address, Ipv4Cidr, StackResources, StaticConfigV4};

/// Network stack resources per node
pub struct NodeNetworkResources {
    pub resources: StackResources<3>, // 3 sockets max per node
}

impl NodeNetworkResources {
    pub const fn new() -> Self {
        Self {
            resources: StackResources::new(),
        }
    }
}

/// Get network configuration for a node
pub fn get_node_config(node_id: u8) -> Config {
    Config::ipv4_static(StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Address::new(10, 0, 0, node_id), 24),
        gateway: Some(Ipv4Address::new(10, 0, 0, 254)),
        dns_servers: Default::default(),
    })
}

/// Static storage for all 5 node network resources
pub static mut NODE_RESOURCES: [MaybeUninit<NodeNetworkResources>; 5] = [
    MaybeUninit::uninit(),
    MaybeUninit::uninit(),
    MaybeUninit::uninit(),
    MaybeUninit::uninit(),
    MaybeUninit::uninit(),
];

/// Get resources for a specific node (must be called once per node)
#[allow(static_mut_refs)]
pub unsafe fn get_node_resources(node_id: u8) -> &'static mut NodeNetworkResources {
    let idx = (node_id - 1) as usize;
    NODE_RESOURCES[idx].write(NodeNetworkResources::new())
}
