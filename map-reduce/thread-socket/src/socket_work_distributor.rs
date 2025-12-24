use crate::socket_completion_signaling::{CompletionSender, SocketCompletionSignaling};
use map_reduce_core::shutdown_signal::ShutdownSignal;
use map_reduce_core::worker::Worker;
use std::collections::HashMap;
use std::mem;
use std::time::{Duration, Instant};

/// Assignment tracking information
#[derive(Clone)]
struct AssignmentInfo<A> {
    assignment: A,
    start_time: Instant,
}

/// Socket-based work distributor
pub struct SocketWorkDistributor<W, F>
where
    W: Worker,
    F: FnMut(usize) -> W + Send,
{
    worker_factory: F,
    timeout: Option<Duration>,
    _phantom: std::marker::PhantomData<W>,
}

impl<W, F> SocketWorkDistributor<W, F>
where
    W: Worker<Completion = CompletionSender>,
    W::Assignment: Clone,
    F: FnMut(usize) -> W + Send,
{
    pub fn with_timeout(worker_factory: F, timeout_ms: u64) -> Self {
        Self {
            worker_factory,
            timeout: if timeout_ms > 0 {
                Some(Duration::from_millis(timeout_ms))
            } else {
                None
            },
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn distribute_work<SD: ShutdownSignal>(
        &mut self,
        mut workers: Vec<W>,
        assignments: Vec<W::Assignment>,
        signaling: &SocketCompletionSignaling,
        shutdown_signal: &SD,
    ) -> Vec<W> {
        if assignments.is_empty() {
            return workers;
        }

        let mut assignment_index = 0;
        let mut active_workers = 0;
        let mut worker_assignments: HashMap<usize, AssignmentInfo<W::Assignment>> = HashMap::new();

        // Distribute initial assignments
        for worker_id in 0..workers.len().min(assignments.len()) {
            let assignment = assignments[assignment_index].clone();
            let sender = signaling.get_sender(worker_id);
            workers[worker_id].send_work(assignment.clone(), sender);
            worker_assignments.insert(
                worker_id,
                AssignmentInfo {
                    assignment,
                    start_time: Instant::now(),
                },
            );
            assignment_index += 1;
            active_workers += 1;
        }

        // Process completions and reassignments
        while active_workers > 0 {
            // Check for shutdown signal
            if shutdown_signal.is_cancelled() {
                println!("Distributor received shutdown signal, stopping work distribution");
                break;
            }

            // Check for stragglers if timeout is configured
            if let Some(timeout) = self.timeout {
                let mut stragglers = Vec::new();
                for (worker_id, info) in &worker_assignments {
                    if info.start_time.elapsed() > timeout {
                        stragglers.push(*worker_id);
                    }
                }

                // Handle stragglers
                for worker_id in stragglers {
                    if let Some(info) = worker_assignments.remove(&worker_id) {
                        eprintln!("⏱️  Worker {} is a straggler (timeout exceeded)! Respawning and reassigning work...", worker_id);

                        // Replace worker
                        let failed_worker =
                            mem::replace(&mut workers[worker_id], (self.worker_factory)(worker_id));
                        drop(failed_worker);

                        // Drain pending messages
                        signaling.drain_worker(worker_id);

                        // Reassign work
                        let sender = signaling.get_sender(worker_id);
                        workers[worker_id].send_work(info.assignment.clone(), sender);
                        worker_assignments.insert(
                            worker_id,
                            AssignmentInfo {
                                assignment: info.assignment,
                                start_time: Instant::now(),
                            },
                        );
                    }
                }
            }

            // Wait for next completion
            if let Some(result) = signaling.wait_next() {
                match result {
                    Ok(worker_id) => {
                        // Success
                        worker_assignments.remove(&worker_id);
                        active_workers -= 1;

                        // Assign next work if available
                        if assignment_index < assignments.len() {
                            let assignment = assignments[assignment_index].clone();
                            let sender = signaling.get_sender(worker_id);
                            workers[worker_id].send_work(assignment.clone(), sender);
                            worker_assignments.insert(
                                worker_id,
                                AssignmentInfo {
                                    assignment,
                                    start_time: Instant::now(),
                                },
                            );
                            assignment_index += 1;
                            active_workers += 1;
                        }
                    }
                    Err(worker_id) => {
                        // Failure - respawn and reassign
                        println!(
                            "⚠️  Worker {} failed! Respawning and reassigning work...",
                            worker_id
                        );

                        if let Some(info) = worker_assignments.get(&worker_id).cloned() {
                            // Replace worker
                            let failed_worker = mem::replace(
                                &mut workers[worker_id],
                                (self.worker_factory)(worker_id),
                            );
                            drop(failed_worker);

                            // Drain pending messages
                            signaling.drain_worker(worker_id);

                            // Reassign work
                            let sender = signaling.get_sender(worker_id);
                            workers[worker_id].send_work(info.assignment.clone(), sender);
                            worker_assignments.insert(
                                worker_id,
                                AssignmentInfo {
                                    assignment: info.assignment,
                                    start_time: Instant::now(),
                                },
                            );
                        }
                    }
                }
            }
        }

        workers
    }
}
