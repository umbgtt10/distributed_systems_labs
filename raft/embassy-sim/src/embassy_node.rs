// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use alloc::string::String;
use embassy_futures::select::{select4, Either4};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Receiver;
use embassy_time::Duration;

use crate::cancellation_token::CancellationToken;
use crate::cluster::ClientRequest;
use crate::embassy_log_collection::EmbassyLogEntryCollection;
use crate::embassy_map_collection::EmbassyMapCollection;
use crate::embassy_node_collection::EmbassyNodeCollection;
use crate::embassy_observer::EmbassyObserver;
use crate::embassy_state_machine::EmbassyStateMachine;
use crate::embassy_storage::EmbassyStorage;
use crate::embassy_timer::EmbassyTimer;
use crate::led_state::LedState;
use crate::transport::async_transport::AsyncTransport;
use crate::transport::embassy_transport::EmbassyTransport;

use raft_core::election_manager::ElectionManager;
use raft_core::event::Event;
use raft_core::log_replication_manager::LogReplicationManager;
use raft_core::node_collection::NodeCollection;
use raft_core::observer::EventLevel;
use raft_core::raft_node::RaftNode;
use raft_core::raft_node_builder::RaftNodeBuilder;
use raft_core::timer_service::TimerService;
use raft_core::types::NodeId;

/// Embassy Raft node - encapsulates all node state and behavior
pub struct EmbassyNode<T: AsyncTransport> {
    node_id: NodeId,
    raft_node: RaftNode<
        EmbassyTransport,
        EmbassyStorage,
        String,
        EmbassyStateMachine,
        EmbassyNodeCollection,
        EmbassyLogEntryCollection,
        EmbassyMapCollection,
        EmbassyTimer,
        EmbassyObserver<String, EmbassyLogEntryCollection>,
    >,
    transport: EmbassyTransport,
    async_transport: T,
    client_rx: Receiver<'static, CriticalSectionRawMutex, ClientRequest, 4>,
    led: LedState,
}

impl<T: AsyncTransport> EmbassyNode<T> {
    /// Create a new Embassy Raft node
    pub fn new(
        node_id: NodeId,
        async_transport: T,
        client_rx: Receiver<'static, CriticalSectionRawMutex, ClientRequest, 4>,
        observer_level: EventLevel,
    ) -> Self {
        info!("Node {} initializing...", node_id);

        let storage = EmbassyStorage::new();
        let timer = EmbassyTimer::new();
        let transport = EmbassyTransport::new();
        let state_machine = EmbassyStateMachine::default();
        let led = LedState::new(node_id as u8);

        // Create peer collection
        let mut peers = EmbassyNodeCollection::new();
        for id in 1..=5 {
            if id != node_id {
                let _ = peers.push(id);
            }
        }

        // Create election manager with timer
        let election = ElectionManager::new(timer);

        // Create log replication manager
        let replication = LogReplicationManager::<EmbassyMapCollection>::new();

        // Create observer with configured level
        let observer = EmbassyObserver::<String, EmbassyLogEntryCollection>::new(observer_level);

        // Build RaftNode using builder pattern
        let raft_node = RaftNodeBuilder::new(node_id, storage, state_machine)
            .with_election(election)
            .with_replication(replication)
            .with_transport(transport.clone(), peers, observer);

        info!("Node {} initialized as Follower", node_id);

        Self {
            node_id,
            raft_node,
            transport,
            async_transport,
            client_rx,
            led,
        }
    }

    /// Run the node's main event loop (concurrent event-driven)
    pub async fn run(mut self, cancel: CancellationToken) {
        self.led.update(self.raft_node.role());

        loop {
            // Create futures for all event sources
            let cancel_fut = cancel.wait();
            let client_fut = self.client_rx.receive();

            // Timer future - polls for expired timers
            let timer_fut = async {
                loop {
                    let timer_service = self.raft_node.timer_service();
                    let expired_timers = timer_service.check_expired();

                    if let Some(timer_kind) = expired_timers.iter().next() {
                        return timer_kind;
                    }

                    embassy_time::Timer::after(Duration::from_millis(10)).await;
                }
            };

            let transport_fut = self.async_transport.recv();

            let event = select4(cancel_fut, client_fut, timer_fut, transport_fut).await;

            match event {
                Either4::First(_) => {
                    // Shutdown signal received
                    info!("Node {} shutting down gracefully", self.node_id);
                    break;
                }
                Either4::Second(request) => {
                    // Client request received
                    self.handle_client_request(request).await;
                }
                Either4::Third(timer_kind) => {
                    // Timer expired
                    self.raft_node.on_event(Event::TimerFired(timer_kind));
                    self.led.update(self.raft_node.role());
                }
                Either4::Fourth((from, msg)) => {
                    // Message received from peer
                    self.raft_node.on_event(Event::Message { from, msg });
                    self.led.update(self.raft_node.role());
                }
            }

            // After any event, drain outbox and send messages
            self.drain_and_send_messages().await;
        }

        info!("Node {} shutdown complete", self.node_id);
    }

    /// Drain outbox and send all messages
    async fn drain_and_send_messages(&mut self) {
        for (target, msg) in self.transport.drain_outbox() {
            self.async_transport.send(target, msg).await;
        }
    }

    /// Handle client request
    async fn handle_client_request(&mut self, request: ClientRequest) {
        match request {
            ClientRequest::Write { payload } => {
                info!("Node {} received client command: {}", self.node_id, payload);
                // TODO: Forward to RaftNode for processing
                // Will need to add client command handling to raft_core
            }
            ClientRequest::GetLeader => {
                // TODO: Respond with current leader
            }
        }
    }
}
