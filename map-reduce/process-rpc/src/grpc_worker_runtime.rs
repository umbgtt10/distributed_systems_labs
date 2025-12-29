use map_reduce_core::worker_runtime::{WorkerRuntime, WorkerTask};
use serde::{de::DeserializeOwned, Serialize};
use std::process::Stdio;
use tokio::process::{Child, Command};

pub struct AutoKillChild(Child);

impl Drop for AutoKillChild {
    fn drop(&mut self) {
        let _ = self.0.start_kill();
    }
}

#[derive(Clone)]
pub struct MapperProcessRuntime;

impl<T> WorkerRuntime<T> for MapperProcessRuntime
where
    T: WorkerTask<Output = ()> + Serialize + DeserializeOwned + Send + 'static,
{
    type Handle = AutoKillChild;
    type Error = std::io::Error;

    fn spawn(task: T) -> Self::Handle {
        let exe = std::env::current_exe().expect("Failed to get current exe");
        let task_json = serde_json::to_string(&task).expect("Failed to serialize task");

        let child = Command::new(exe)
            .arg("--worker")
            .arg("--type")
            .arg("mapper")
            .arg("--task")
            .arg(task_json)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("Failed to spawn mapper process");

        AutoKillChild(child)
    }

    async fn join(mut handle: Self::Handle) -> Result<(), Self::Error> {
        handle.0.wait().await.map(|_| ())
    }
}

#[derive(Clone)]
pub struct ReducerProcessRuntime;

impl<T> WorkerRuntime<T> for ReducerProcessRuntime
where
    T: WorkerTask<Output = ()> + Serialize + DeserializeOwned + Send + 'static,
{
    type Handle = AutoKillChild;
    type Error = std::io::Error;

    fn spawn(task: T) -> Self::Handle {
        let exe = std::env::current_exe().expect("Failed to get current exe");
        let task_json = serde_json::to_string(&task).expect("Failed to serialize task");

        let child = Command::new(exe)
            .arg("--worker")
            .arg("--type")
            .arg("reducer")
            .arg("--task")
            .arg(task_json)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("Failed to spawn reducer process");

        AutoKillChild(child)
    }

    async fn join(mut handle: Self::Handle) -> Result<(), Self::Error> {
        handle.0.wait().await.map(|_| ())
    }
}
