use std::future::Future;
use std::future::pending;

use tokio::task::JoinError;
use tokio::task::JoinSet;
use tracing::warn;

pub(crate) struct ConnectionCleanupTasks {
    tasks: JoinSet<()>,
}

impl ConnectionCleanupTasks {
    pub(crate) fn new() -> Self {
        Self {
            tasks: JoinSet::new(),
        }
    }

    pub(crate) fn spawn(&mut self, future: impl Future<Output = ()> + Send + 'static) {
        self.tasks.spawn(future);
    }

    pub(crate) async fn reap_next(&mut self) {
        if self.tasks.is_empty() {
            pending::<()>().await;
        }
        if let Some(result) = self.tasks.join_next().await {
            log_cleanup_result(result);
        }
    }

    pub(crate) async fn drain(&mut self) {
        while let Some(result) = self.tasks.join_next().await {
            log_cleanup_result(result);
        }
    }

    pub(crate) fn abort(&mut self) {
        self.tasks.abort_all();
    }
}

fn log_cleanup_result(result: Result<(), JoinError>) {
    if let Err(err) = result
        && !err.is_cancelled()
    {
        warn!("connection cleanup task failed: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    use tokio::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn drain_waits_for_spawned_tasks() {
        let ran = Arc::new(AtomicBool::new(false));
        let ran_clone = Arc::clone(&ran);
        let mut tasks = ConnectionCleanupTasks::new();

        tasks.spawn(async move {
            ran_clone.store(true, Ordering::Release);
        });

        timeout(Duration::from_secs(1), tasks.drain())
            .await
            .expect("cleanup drain should finish");
        assert!(ran.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn reap_next_waits_when_empty() {
        let mut tasks = ConnectionCleanupTasks::new();

        timeout(Duration::from_millis(50), tasks.reap_next())
            .await
            .expect_err("empty reap should wait for future work");
    }
}
