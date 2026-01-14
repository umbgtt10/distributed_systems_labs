use crate::{
    in_memory_state_machine::InMemoryStateMachine, in_memory_storage::InMemoryStorage,
    in_memory_transport::InMemoryTransport, vec_node_collection::VecNodeCollection,
};
use raft_core::{raft_node::RaftNode, types::NodeId};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub type RaftCluster = Arc<
    Mutex<
        HashMap<
            NodeId,
            RaftNode<
                InMemoryTransport,
                InMemoryStorage,
                String,
                InMemoryStateMachine,
                VecNodeCollection,
            >,
        >,
    >,
>;
