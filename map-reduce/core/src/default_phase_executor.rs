use crate::completion_signaling::CompletionSignaling;
use crate::worker::Worker;
use std::collections::HashMap;
use std::mem;
use std::time::{Duration, Instant};

/// Assignment tracking information
#[derive(Clone)]
struct AssignmentInfo<A> {
    assignment: A,
    start_time: Instant,
}

/// Default phase executor with fault tolerance and straggler detection
/// Generic over worker type, completion signaling, and worker factory
pub struct DefaultPhaseExecutor<W, CS, F>
where
    W: Worker,
    CS: CompletionSignaling,
    F: FnMut(usize) -> W + Send,
{
    worker_factory: F,
    timeout: Option<Duration>,
    _phantom: std::marker::PhantomData<(W, CS)>,
}

impl<W, CS, F> DefaultPhaseExecutor<W, CS, F>
where
    W: Worker,
    CS: CompletionSignaling,
    F: FnMut(usize) -> W + Send,
{
    pub fn new(worker_factory: F, timeout_ms: u64) -> Self {
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

    pub async fn execute(
        &mut self,
        mut workers: Vec<W>,
        assignments: Vec<W::Assignment>,
        mut signaling: CS,
        get_completion: impl Fn(&CS, usize) -> CS::Token,
    ) -> Vec<W>
    where
        W::Assignment: Clone,
        W::Completion: From<CS::Token>,
    {
        if assignments.is_empty() {
            return workers;
        }

        let mut assignment_index = 0;
        let mut active_workers = 0;
        let mut worker_assignments: HashMap<usize, AssignmentInfo<W::Assignment>> = HashMap::new();

        // Distribute initial assignments
        for (worker_id, worker) in workers.iter().enumerate().take(assignments.len()) {
            let assignment = assignments[assignment_index].clone();
            let completion = get_completion(&signaling, worker_id);
            worker.send_work(assignment.clone(), completion.into());
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
            // Check for stragglers if timeout is configured
            if let Some(timeout_duration) = self.timeout {
                let mut stragglers = Vec::new();
                for (worker_id, info) in &worker_assignments {
                    if info.start_time.elapsed() > timeout_duration {
                        stragglers.push(*worker_id);
                    }
                }

                // Handle stragglers
                for worker_id in stragglers {
                    if let Some(info) = worker_assignments.remove(&worker_id) {
                        eprintln!(
                            "⏱️  Worker {} is a straggler (timeout exceeded)! Respawning and reassigning work...",
                            worker_id
                        );

                        // Replace worker
                        let failed_worker =
                            mem::replace(&mut workers[worker_id], (self.worker_factory)(worker_id));
                        drop(failed_worker);

                        // Drain pending messages and replace signaling token
                        signaling.drain_worker(worker_id).await;
                        let completion_token = signaling.replace_worker(worker_id);

                        // Reassign work
                        workers[worker_id]
                            .send_work(info.assignment.clone(), completion_token.into());
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

            // Wait for completion
            // If we have timeout configured, use short timeout to check stragglers
            // Otherwise wait indefinitely for completion
            let use_timeout = self.timeout.is_some();

            let completion_result = if use_timeout {
                let wait_duration = self.timeout.unwrap() / 10;

                // Use timeout to periodically check for stragglers
                match tokio::time::timeout(wait_duration, signaling.wait_next()).await {
                    Ok(result) => result,
                    Err(_) => {
                        // Timeout occurred - loop will check for stragglers
                        continue;
                    }
                }
            } else {
                // No timeout needed - just wait for next completion
                signaling.wait_next().await
            };

            if let Some(result) = completion_result {
                match result {
                    Ok(worker_id) => {
                        // Worker completed successfully
                        worker_assignments.remove(&worker_id);
                        active_workers -= 1;

                        // Assign next assignment if available
                        if assignment_index < assignments.len() {
                            let assignment = assignments[assignment_index].clone();
                            let completion = get_completion(&signaling, worker_id);
                            workers[worker_id].send_work(assignment.clone(), completion.into());
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
                        // Worker failed - respawn and reassign
                        eprintln!(
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

                            // Drain pending messages and replace signaling token
                            signaling.drain_worker(worker_id).await;
                            let completion_token = signaling.replace_worker(worker_id);

                            // Reassign work
                            workers[worker_id]
                                .send_work(info.assignment.clone(), completion_token.into());
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
