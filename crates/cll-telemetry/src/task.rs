use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use tokio::time;

pub struct TaskWithDelayThread {
    delay: Duration,
    function: Box<dyn Fn() + Send + 'static>,
    stopping: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl TaskWithDelayThread {
    pub fn new<F>(delay: Duration, function: F) -> Self
    where
        F: Fn() + Send + 'static,
    {
        TaskWithDelayThread {
            delay,
            function: Box::new(function),
            stopping: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(Notify::new()),
        }
    }

    pub fn request_run(&self, immediate: bool) {
        if immediate {
            (self.function)();
        } else {
            self.notify.notify_one();
        }
    }

    pub fn is_stopping(&self) -> bool {
        self.stopping.load(Ordering::SeqCst)
    }

    pub fn terminate(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    pub async fn run_loop(&self) {
        loop {
            if self.stopping.load(Ordering::SeqCst) {
                break;
            }

            tokio::select! {
                _ = self.notify.notified() => {},
                _ = time::sleep(self.delay) => {},
            }

            if self.stopping.load(Ordering::SeqCst) {
                break;
            }

            (self.function)();
        }
    }

    pub fn run_loop_sync(&self) {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(self.run_loop());
    }
}

impl Drop for TaskWithDelayThread {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }
}
