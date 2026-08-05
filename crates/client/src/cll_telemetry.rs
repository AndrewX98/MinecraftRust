use std::ffi::CStr;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use cll_telemetry::{
    Event, EventFlags, EventManager, EventUploadRequest, EventUploadStep,
};

/// Game version used for the telemetry envelope. Mirrors the deleted C++
/// `MinecraftVersion::getString()` (mcpelauncher-core) — hardcoded to the
/// currently supported game dir. Nothing uploads while running as a PlayFab
/// guest (no MSA tokens), so this is informational only.
const APP_VERSION: &str = "1.26.20";

/// How often the flush thread drains realtime events and uploads stored ones.
const FLUSH_INTERVAL: Duration = Duration::from_secs(15);

/// Singleton holding the Rust `EventManager`, constructed by `init()` (JNI
/// `Interop.initCLL`). Locked by the game thread (`log`) and the flush thread.
static MANAGER: OnceLock<Mutex<Option<EventManager>>> = OnceLock::new();

/// Auth step replacing the C++ `CllUploadAuthStep`. Offline build has no MSA
/// tokens, so it adds no headers and never signals a retryable auth failure.
struct CllUploadAuthStep;

impl EventUploadStep for CllUploadAuthStep {
    fn on_request(&self, _request: &mut EventUploadRequest) {}
    fn on_authentication_failed(&self) -> bool {
        false
    }
}

/// Initialize the telemetry stack (JNI `Interop.initCLL`). Idempotent.
pub fn init(cid: &str) {
    if MANAGER.get().is_some() {
        log::warn!("cll: init called twice (cid={}), ignoring", cid);
        return;
    }

    let dir = crate::path_helper::path_helper_get_primary_data_directory();
    let data_dir = if dir.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(dir).to_string_lossy().into_owned() }
    };
    if data_dir.is_empty() {
        log::warn!("cll: primary data directory unavailable, telemetry disabled");
        return;
    }

    let base = Path::new(&data_dir).join("cll-telemetry");
    let batches_dir = base.join("batches");
    let cache_dir = base.join("cache");
    for d in [&batches_dir, &cache_dir] {
        if let Err(e) = std::fs::create_dir_all(d) {
            log::warn!("cll: failed to create {}: {}", d.display(), e);
        }
    }

    let mut manager = EventManager::new(
        cid,
        &batches_dir.to_string_lossy(),
        &cache_dir.to_string_lossy(),
    );
    manager.set_app("Minecraft", APP_VERSION);
    manager.add_upload_step(Box::new(CllUploadAuthStep));

    match MANAGER.set(Mutex::new(Some(manager))) {
        Ok(()) => {
            log::info!("cll: EventManager initialized (cid={}, batches={}, cache={})",
                cid, batches_dir.display(), cache_dir.display());
            spawn_flush_thread();
        }
        Err(_) => log::warn!("cll: init raced with another init, ignoring"),
    }
}

/// Queue a telemetry event (JNI `Interop.logCLL`).
pub fn log(ticket: &str, name: &str, data: &str) {
    let Some(mutex) = MANAGER.get() else {
        log::warn!("cll: log before init, dropping event '{}'", name);
        return;
    };
    let mut guard = match mutex.lock() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    };
    let Some(manager) = guard.as_mut() else {
        log::warn!("cll: manager unavailable, dropping event '{}'", name);
        return;
    };

    let json = serde_json::from_str(data)
        .unwrap_or_else(|_| serde_json::Value::String(data.to_string()));
    let event = Event::new(
        name,
        json,
        EventFlags::PERSISTENCE_CRITICAL | EventFlags::LATENCY_REALTIME,
    )
    .with_ids(vec![ticket.to_string()]);
    manager.add(event);
    log::debug!("cll: queued event '{}'", name);
}

fn spawn_flush_thread() {
    std::thread::Builder::new()
        .name("cll-flush".to_string())
        .spawn(|| loop {
            std::thread::sleep(FLUSH_INTERVAL);
            let Some(mutex) = MANAGER.get() else {
                continue;
            };
            let mut guard = match mutex.lock() {
                Ok(g) => g,
                Err(e) => e.into_inner(),
            };
            if let Some(manager) = guard.as_mut() {
                manager.upload_realtime();
                manager.upload_storage();
            }
        })
        .expect("failed to spawn cll flush thread");
}
