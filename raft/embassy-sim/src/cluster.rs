// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Raft cluster handle for client interactions

use crate::cancellation_token::CancellationToken;
use alloc::string::String;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Sender};
use embassy_sync::mutex::Mutex;
use embassy_time::Duration;
use raft_core::types::NodeId;

// Shared client infrastructure (transport-agnostic)
pub(crate) static CLIENT_CHANNELS: [Channel<CriticalSectionRawMutex, ClientRequest, 4>; 5] = [
    Channel::new(),
    Channel::new(),
    Channel::new(),
    Channel::new(),
    Channel::new(),
];

pub(crate) static CLIENT_RESPONSE_CHANNEL: Channel<
    CriticalSectionRawMutex,
    Result<(), ClusterError>,
    1,
> = Channel::new();

pub(crate) static CURRENT_LEADER: Mutex<CriticalSectionRawMutex, Option<NodeId>> = Mutex::new(None);

/// Client request to Raft cluster
#[derive(Clone)]
pub enum ClientRequest {
    /// Write command to replicated log
    Write {
        payload: String,
        response_tx: Sender<'static, CriticalSectionRawMutex, Result<(), ClusterError>, 1>,
    },
    /// Get current leader (for client redirection)
    GetLeader,
}

/// Error types for cluster operations
#[derive(Debug, Clone, Copy)]
pub enum ClusterError {
    /// No leader currently elected
    NoLeader,
    /// Channel send failed
    ChannelFull,
    /// Timeout waiting for response
    Timeout,
}

/// Handle to interact with a running Raft cluster
pub struct RaftCluster {
    /// Client channels (one per node for requests)
    client_channels: [Sender<'static, CriticalSectionRawMutex, ClientRequest, 4>; 5],

    /// Leader tracking (updated by nodes via observer)
    current_leader: &'static Mutex<CriticalSectionRawMutex, Option<NodeId>>,

    /// For graceful shutdown
    cancel: CancellationToken,
}

impl RaftCluster {
    /// Create new cluster handle from shared statics
    pub fn new(cancel: CancellationToken) -> Self {
        Self {
            client_channels: [
                CLIENT_CHANNELS[0].sender(),
                CLIENT_CHANNELS[1].sender(),
                CLIENT_CHANNELS[2].sender(),
                CLIENT_CHANNELS[3].sender(),
                CLIENT_CHANNELS[4].sender(),
            ],
            current_leader: &CURRENT_LEADER,
            cancel,
        }
    }

    /// Submit a write command to the cluster
    /// Sends to a random node - Raft protocol handles forwarding to leader
    pub async fn submit_command(&self, payload: String) -> Result<(), ClusterError> {
        // Clear any previous responses
        while CLIENT_RESPONSE_CHANNEL.try_receive().is_ok() {}

        let req = ClientRequest::Write {
            payload,
            response_tx: CLIENT_RESPONSE_CHANNEL.sender(),
        };

        let mut sent = false;
        // Try to send to a random node (or all of them)
        // Since we don't have random here easily, just iterate.
        // It's effectively "try any available node".
        for channel in self.client_channels.iter() {
            if channel.try_send(req.clone()).is_ok() {
                sent = true;
                break;
            }
        }

        if !sent {
            return Err(ClusterError::ChannelFull);
        }

        // Wait for response
        // In a real system, we'd use IDs to match response to request
        match embassy_time::with_timeout(Duration::from_secs(10), CLIENT_RESPONSE_CHANNEL.receive())
            .await
        {
            Ok(result) => result,
            Err(_) => Err(ClusterError::Timeout),
        }
    }

    /// Wait for a leader to be elected
    pub async fn wait_for_leader(&self, timeout: Duration) -> Result<NodeId, ClusterError> {
        let start = embassy_time::Instant::now();

        loop {
            if let Some(leader) = *self.current_leader.lock().await {
                return Ok(leader);
            }

            if embassy_time::Instant::now() - start > timeout {
                return Err(ClusterError::Timeout);
            }

            embassy_time::Timer::after(Duration::from_millis(10)).await;
        }
    }

    /// Initiate graceful shutdown
    pub fn shutdown(&self) {
        self.cancel.cancel();
    }
}
