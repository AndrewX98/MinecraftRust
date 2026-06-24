use crate::batch::{
    BufferedEventBatch, EventBatch, MemoryEventBatch, MultiFileEventBatch,
};
use crate::config::{Configuration, ConfigurationManager, FileConfigurationCache};
use crate::event::{Event, EventFlags};
use crate::serializer::EventSerializer;

use crate::uploader::{EventUploadStep, EventUploader};

pub struct EventManager {
    i_key: String,
    serializer: EventSerializer,
    normal_batch: BufferedEventBatch,
    critical_batch: Box<dyn EventBatch>,
    realtime_memory: MemoryEventBatch,
    uploader: EventUploader,
    config_manager: ConfigurationManager,
    upload_steps: Vec<Box<dyn EventUploadStep + Send + Sync>>,
}

impl EventManager {
    pub fn new(i_key: &str, batches_dir: &str, cache_dir: &str) -> Self {
        let uploader = EventUploader::new("https://vortex.data.microsoft.com/collect/v1");

        let config_cache_path = format!("{}/config_cache.json", cache_dir);
        let mut config_manager = ConfigurationManager::new();
        config_manager.set_cache(Box::new(FileConfigurationCache::new(&config_cache_path)));

        // Add default configurations
        let android_settings_url = format!(
            "https://settings.data.microsoft.com/settings/v2.0/android-{}-settings",
            i_key
        );
        config_manager.add(Configuration::new(&android_settings_url));

        let telemetry_url = format!(
            "https://settings.data.microsoft.com/settings/v2.0/telemetry-{}-settings",
            i_key
        );
        config_manager.add(Configuration::new(&telemetry_url));

        config_manager.load_cached_configs();

        let critical_batch = MultiFileEventBatch::new(batches_dir, "crit", ".jsonl", 500, 1024 * 1024);
        let normal_storage = MultiFileEventBatch::new(batches_dir, "normal", ".jsonl", 500, 1024 * 1024);
        let normal_batch = BufferedEventBatch::new(Box::new(normal_storage), 50);

        EventManager {
            i_key: i_key.to_string(),
            serializer: EventSerializer::new(),
            normal_batch,
            critical_batch: Box::new(critical_batch),
            realtime_memory: MemoryEventBatch::new(50),
            uploader,
            config_manager,
            upload_steps: Vec::new(),
        }
    }

    pub fn add_upload_step(&mut self, step: Box<dyn EventUploadStep + Send + Sync>) {
        self.upload_steps.push(step);
    }

    pub fn set_app(&mut self, app_id: &str, app_ver: &str) {
        self.serializer.set_app(app_id, app_ver);
    }

    pub fn add(&mut self, event: Event) {
        self.serializer.set_i_key(&self.i_key);
        let envelope = self.serializer.create_envelope_for(&event);
        let json = serde_json::to_value(&envelope).unwrap_or(serde_json::Value::Null);

        if event.flags.contains(EventFlags::LATENCY_REALTIME) {
            if !self.realtime_memory.add_event(json.clone()) {
                // Fall through to storage
                if event.flags.contains(EventFlags::PERSISTENCE_CRITICAL) {
                    self.critical_batch.add_event(json);
                } else {
                    self.normal_batch.add_event(json);
                }
            }
        } else if event.flags.contains(EventFlags::PERSISTENCE_CRITICAL) {
            self.critical_batch.add_event(json);
        } else {
            self.normal_batch.add_event(json);
        }
    }

    pub fn upload_realtime(&mut self) {
        let events = self.realtime_memory.transfer_all_events();
        if events.is_empty() {
            return;
        }

        // Build data from events
        let lines: Vec<String> = events.iter()
            .map(|v| serde_json::to_string(v).unwrap_or_default())
            .collect();
        let data = lines.join("\n").into_bytes();

        let batch = crate::batch::VectorBatchedEventList::new(data, events.len(), false);
        let status = self.uploader.send_events(&batch, true);

        if !status.is_success() {
            // Transfer back to disk batches
            for event in events {
                let flags = EventFlags::PERSISTENCE_NORMAL;
                if flags.contains(EventFlags::PERSISTENCE_CRITICAL) {
                    self.critical_batch.add_event(event);
                } else {
                    self.normal_batch.add_event(event);
                }
            }
        }
    }

    pub fn upload_storage(&mut self) {
        // Update config if needed
        self.config_manager.download_configs();

        let max_events = self.config_manager.get_max_events_per_post() as usize;
        let max_size = self.config_manager.get_max_event_size_in_bytes() as usize;

        // Upload critical first
        loop {
            let result = self.uploader.send_events_from_batch(
                self.critical_batch.as_mut(),
                max_events,
                max_size,
            );
            if !result.is_success() || !self.critical_batch.has_events() {
                break;
            }
        }

        // Then normal
        loop {
            let result = self.uploader.send_events_from_batch(
                &mut self.normal_batch,
                max_events,
                max_size,
            );
            if !result.is_success() || !self.normal_batch.has_events() {
                break;
            }
        }
    }
}
