// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Mock network driver for UDP simulation
//!
//! This driver simulates Ethernet by using shared in-memory message queues
//! between nodes. Each node has a virtual MAC address and can send/receive
//! Ethernet frames through a shared bus.

use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::task::Waker;
use embassy_net_driver::{Capabilities, Driver, HardwareAddress, LinkState, RxToken, TxToken};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::channel::Channel;

const MTU: usize = 1500;
const CHANNEL_SIZE: usize = 16; // Packet queue depth per node

/// Shared bus for all network traffic
/// Each node sends packets here, addressed by MAC
pub struct NetworkBus {
    // One queue per node (indexed by last MAC byte - 1)
    // Node 1 (MAC ending in 01) uses queues[0], etc.
    queues: [Channel<CriticalSectionRawMutex, Packet, CHANNEL_SIZE>; 5],
    // Wakers to notify nodes when packets arrive
    wakers: [Mutex<CriticalSectionRawMutex, RefCell<Option<Waker>>>; 5],
}

#[derive(Clone)]
pub struct Packet {
    pub data: Vec<u8>,
    pub len: usize,
}

impl NetworkBus {
    pub const fn new() -> Self {
        Self {
            queues: [
                Channel::new(),
                Channel::new(),
                Channel::new(),
                Channel::new(),
                Channel::new(),
            ],
            wakers: [
                Mutex::new(RefCell::new(None)),
                Mutex::new(RefCell::new(None)),
                Mutex::new(RefCell::new(None)),
                Mutex::new(RefCell::new(None)),
                Mutex::new(RefCell::new(None)),
            ],
        }
    }

    /// Get sender for a specific node's queue
    fn get_sender(&self, node_id: u8) -> &Channel<CriticalSectionRawMutex, Packet, CHANNEL_SIZE> {
        &self.queues[(node_id - 1) as usize]
    }

    /// Get receiver for a specific node's queue
    fn get_receiver(&self, node_id: u8) -> &Channel<CriticalSectionRawMutex, Packet, CHANNEL_SIZE> {
        &self.queues[(node_id - 1) as usize]
    }

    /// Register a waker for a node
    fn register_waker(&self, node_id: u8, waker: &Waker) {
        self.wakers[(node_id - 1) as usize].lock(|cell| {
            let mut w = cell.borrow_mut();
            if match w.as_ref() {
                Some(old_waker) => !old_waker.will_wake(waker),
                None => true,
            } {
                *w = Some(waker.clone());
            }
        });
    }

    /// Wake a node
    fn wake_node(&self, node_id: u8) {
        self.wakers[(node_id - 1) as usize].lock(|cell| {
            if let Some(waker) = cell.borrow_mut().take() {
                waker.wake();
            }
        });
    }
}

/// Mock network driver implementing embassy-net Driver trait
pub struct MockNetDriver {
    node_id: u8,
    mac_addr: [u8; 6],
    bus: &'static NetworkBus,
}

impl MockNetDriver {
    pub fn new(node_id: u8, bus: &'static NetworkBus) -> Self {
        let mac_addr = [0x02, 0x00, 0x00, 0x00, 0x00, node_id];
        Self {
            node_id,
            mac_addr,
            bus,
        }
    }

    /// Try to poll for a received packet (non-blocking)
    fn try_recv_packet(&self) -> Option<Packet> {
        let receiver = self.bus.get_receiver(self.node_id);
        receiver.try_receive().ok()
    }
}

impl Driver for MockNetDriver {
    type RxToken<'a>
        = MockRxToken
    where
        Self: 'a;
    type TxToken<'a>
        = MockTxToken<'a>
    where
        Self: 'a;

    fn receive(
        &mut self,
        cx: &mut core::task::Context,
    ) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        crate::info!("Node {} receive polling", self.node_id);
        // Try to get a packet from our queue
        if let Some(packet) = self.try_recv_packet() {
            let rx = MockRxToken {
                packet: Some(packet),
            };
            let tx = MockTxToken {
                node_id: self.node_id,
                bus: self.bus,
            };

            Some((rx, tx))
        } else {
            // No packet, register waker so we get notified when one arrives
            self.bus.register_waker(self.node_id, cx.waker());
            None
        }
    }

    fn transmit(&mut self, _cx: &mut core::task::Context) -> Option<Self::TxToken<'_>> {
        // Always ready to transmit
        crate::info!("Node {} transmit ready check", self.node_id);
        Some(MockTxToken {
            node_id: self.node_id,
            bus: self.bus,
        })
    }

    fn link_state(&mut self, _cx: &mut core::task::Context) -> LinkState {
        LinkState::Up // Always connected
    }

    fn capabilities(&self) -> Capabilities {
        let mut caps = Capabilities::default();
        caps.max_transmission_unit = MTU;
        caps.max_burst_size = Some(1);
        caps
    }

    fn hardware_address(&self) -> HardwareAddress {
        HardwareAddress::Ethernet(self.mac_addr)
    }
}

/// RX token for receiving packets
pub struct MockRxToken {
    packet: Option<Packet>,
}

impl RxToken for MockRxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        if let Some(mut pkt) = self.packet {
            f(&mut pkt.data[..pkt.len])
        } else {
            f(&mut [])
        }
    }
}

/// TX token for sending packets
pub struct MockTxToken<'a> {
    node_id: u8,
    bus: &'a NetworkBus,
}

impl<'a> TxToken for MockTxToken<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buffer = vec![0u8; len];
        let result = f(&mut buffer);

        // Parse destination MAC from Ethernet header (first 6 bytes)
        if buffer.len() >= 6 {
            let dest_mac = [
                buffer[0], buffer[1], buffer[2], buffer[3], buffer[4], buffer[5],
            ];

            // Broadcast (FF:FF:FF:FF:FF:FF) or multicast
            if dest_mac[0] == 0xFF {
                // Send to all nodes except sender
                crate::info!("Node {} broadcasting {} bytes", self.node_id, len);
                for target_id in 1..=5 {
                    if target_id != self.node_id {
                        let packet = Packet {
                            data: buffer.clone(),
                            len,
                        };
                        if self.bus.get_sender(target_id).try_send(packet).is_ok() {
                            self.bus.wake_node(target_id);
                        } else {
                            crate::info!(
                                "Node {} failed to send broadcast to {}",
                                self.node_id,
                                target_id
                            );
                        }
                    }
                }
            } else {
                // Unicast - extract target node ID from last byte of MAC
                let target_id = dest_mac[5];
                if target_id >= 1 && target_id <= 5 {
                    crate::info!(
                        "Node {} unicasting {} bytes to {}",
                        self.node_id,
                        len,
                        target_id
                    );
                    let packet = Packet { data: buffer, len };
                    if self.bus.get_sender(target_id).try_send(packet).is_ok() {
                        self.bus.wake_node(target_id);
                    } else {
                        crate::info!(
                            "Node {} failed to send unicast to {}",
                            self.node_id,
                            target_id
                        );
                    }
                }
            }
        }

        result
    }
}
