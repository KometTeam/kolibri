use std::ops::Deref;

use tokio::runtime::{Builder, Runtime};

/// Multi-thread tokio runtime that shuts down in the background when dropped.
///
/// `Runtime`'s own drop blocks until every `spawn_blocking` task returns, and
/// the resolver runs `getaddrinfo` there. On a network that swallows DNS the
/// last handle — released by Dart on its UI isolate — froze the app for the
/// whole resolver timeout. Background shutdown leaves those threads to finish
/// on their own.
pub(crate) struct BackgroundRuntime(Option<Runtime>);

impl BackgroundRuntime {
    pub(crate) fn new() -> Result<Self, String> {
        Builder::new_multi_thread()
            .enable_all()
            .build()
            .map(|rt| Self(Some(rt)))
            .map_err(|e| e.to_string())
    }
}

impl Deref for BackgroundRuntime {
    type Target = Runtime;

    fn deref(&self) -> &Runtime {
        self.0.as_ref().expect("runtime is taken only on drop")
    }
}

impl Drop for BackgroundRuntime {
    fn drop(&mut self) {
        if let Some(rt) = self.0.take() {
            rt.shutdown_background();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use super::BackgroundRuntime;

    #[test]
    fn runs_work_like_a_runtime() {
        let rt = BackgroundRuntime::new().unwrap();
        assert_eq!(rt.block_on(async { 2 + 2 }), 4);
    }

    #[test]
    fn drop_does_not_wait_for_a_stuck_blocking_task() {
        let rt = BackgroundRuntime::new().unwrap();
        let (release, stuck) = mpsc::channel::<()>();
        let (started, running) = mpsc::channel();
        rt.spawn_blocking(move || {
            started.send(()).unwrap();
            let _ = stuck.recv_timeout(Duration::from_secs(10));
        });
        running.recv().unwrap();

        let dropping = Instant::now();
        drop(rt);
        assert!(dropping.elapsed() < Duration::from_secs(1));
        drop(release);
    }
}
