// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use embassy_time::{Duration, Timer};

use crate::cancellation_token::CancellationToken;
use crate::embassy_map_collection::EmbassyMapCollection;
use crate::embassy_node_collection::EmbassyNodeCollection;
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
use raft_core::raft_node_builder::RaftNodeBuilder;
use raft_core::timer_service::TimerService;
use raft_core::types::NodeId;

/// Generic Raft node task implementation
pub async fn raft_node_task_impl<T: AsyncTransport>(
    node_id: NodeId,
    mut async_transport: T,
    cancel: CancellationToken,
) {
    info!("Node {} starting...", node_id);

    let storage = EmbassyStorage::new();
    let timer = EmbassyTimer::new();
    let mut transport = EmbassyTransport::new();
    let state_machine = EmbassyStateMachine::default();
    let mut led = LedState::new(node_id as u8);

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

    // Build RaftNode using builder pattern
    let mut node = RaftNodeBuilder::new(node_id, storage, state_machine)
        .with_election(election)
        .with_replication(replication)
        .with_transport(transport.clone(), peers);

    info!("Node {} initialized as Follower", node_id);
    led.update(node.role());

    loop {
        // Check for cancellation
        if cancel.is_cancelled() {
            info!("Node {} shutting down gracefully", node_id);
            break;
        }

        // 1. Check for timer expiration
        let timer_service = node.timer_service();
        let expired_timers = timer_service.check_expired();

        for timer_kind in expired_timers.iter() {
            info!("Node {} timer fired: {:?}", node_id, timer_kind);
            node.on_event(Event::TimerFired(timer_kind));
            led.update(node.role());
        }

        // 2. Drain outbox and send messages via async transport
        for (target, msg) in transport.drain_outbox() {
            async_transport.send(target, msg).await;
        }

        // 3. Check for incoming messages (non-blocking with timeout)
        let recv_result =
            embassy_time::with_timeout(Duration::from_millis(10), async_transport.recv()).await;

        if let Ok((from, msg)) = recv_result {
            node.on_event(Event::Message { from, msg });
            led.update(node.role());
        }

        // Small yield to allow other tasks to run
        Timer::after(Duration::from_millis(1)).await;
    }

    info!("Node {} shutdown complete", node_id);
}
