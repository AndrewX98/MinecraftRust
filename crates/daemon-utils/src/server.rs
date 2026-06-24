use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownPolicy {
    NoConnections,
    Never,
}

pub struct AutoShutdownService {
    policy: ShutdownPolicy,
    connections: Arc<std::sync::atomic::AtomicI32>,
    notify: Arc<Notify>,
    shutdown_requested: Arc<AtomicBool>,
}

impl AutoShutdownService {
    pub fn new(policy: ShutdownPolicy) -> Self {
        AutoShutdownService {
            policy,
            connections: Arc::new(std::sync::atomic::AtomicI32::new(0)),
            notify: Arc::new(Notify::new()),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn on_client_connected(&self) {
        self.connections.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn on_client_disconnected(&self) {
        let prev = self.connections.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        if prev == 1 && self.policy == ShutdownPolicy::NoConnections {
            self.request_stop();
        }
    }

    pub fn request_stop(&self) {
        self.shutdown_requested.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    pub fn is_stop_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::SeqCst)
    }

    pub async fn run(&self) {
        loop {
            self.notify.notified().await;
            if self.shutdown_requested.load(Ordering::SeqCst) {
                break;
            }
        }
    }
}
