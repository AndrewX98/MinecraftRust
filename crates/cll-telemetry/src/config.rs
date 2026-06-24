use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct ConfigurationProperty<T> {
    value: Option<T>,
}

impl<T> ConfigurationProperty<T> {
    pub fn new() -> Self {
        ConfigurationProperty { value: None }
    }

    pub fn set(&mut self, value: T) {
        self.value = Some(value);
    }

    pub fn get(&self) -> Option<&T> {
        self.value.as_ref()
    }

    pub fn is_set(&self) -> bool {
        self.value.is_some()
    }

    pub fn reset(&mut self) {
        self.value = None;
    }
}

impl<T: Default + Clone> ConfigurationProperty<T> {
    pub fn get_or_default(&self) -> T {
        self.value.as_ref().cloned().unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedConfiguration {
    pub expires: u64,
    pub refresh_interval: u64,
    pub etag: String,
    pub data: serde_json::Value,
}

pub trait ConfigurationCache: Send {
    fn read_from_cache(&self, url: &str) -> Option<CachedConfiguration>;
    fn write_config_to_cache(&self, url: &str, config: &CachedConfiguration);
}

pub struct FileConfigurationCache {
    path: String,
    cache: HashMap<String, CachedConfiguration>,
}

impl FileConfigurationCache {
    pub fn new(path: &str) -> Self {
        let cache = if Path::new(path).exists() {
            fs::read_to_string(path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            HashMap::new()
        };
        FileConfigurationCache {
            path: path.to_string(),
            cache,
        }
    }

    #[allow(dead_code)]
    fn save(&self) {
        if let Ok(data) = serde_json::to_string(&self.cache) {
            if let Some(parent) = Path::new(&self.path).parent() {
                fs::create_dir_all(parent).ok();
            }
            fs::write(&self.path, &data).ok();
        }
    }
}

impl ConfigurationCache for FileConfigurationCache {
    fn read_from_cache(&self, url: &str) -> Option<CachedConfiguration> {
        self.cache.get(url).cloned()
    }

    fn write_config_to_cache(&self, url: &str, config: &CachedConfiguration) {
        // Use interior mutability via file save
        let mut cache = self.cache.clone();
        cache.insert(url.to_string(), config.clone());
        if let Ok(data) = serde_json::to_string(&cache) {
            if let Some(parent) = Path::new(&self.path).parent() {
                fs::create_dir_all(parent).ok();
            }
            fs::write(&self.path, &data).ok();
        }
    }
}

pub struct Configuration {
    pub url: String,
    pub downloaded: bool,
    pub expires: SystemTime,
    // Config properties
    pub max_event_size_in_bytes: ConfigurationProperty<i32>,
    pub max_events_per_post: ConfigurationProperty<i32>,
    pub queue_drain_interval: ConfigurationProperty<i32>,
}

impl Configuration {
    pub fn new(url: &str) -> Self {
        Configuration {
            url: url.to_string(),
            downloaded: false,
            expires: SystemTime::now(),
            max_event_size_in_bytes: ConfigurationProperty::new(),
            max_events_per_post: ConfigurationProperty::new(),
            queue_drain_interval: ConfigurationProperty::new(),
        }
    }

    pub fn needs_redownload(&self) -> bool {
        !self.downloaded || SystemTime::now() > self.expires
    }

    pub fn load_from_cache(&mut self, cache: &dyn ConfigurationCache) -> bool {
        if let Some(cached) = cache.read_from_cache(&self.url) {
            self.parse_config(&cached.data);
            self.downloaded = true;
            self.expires = UNIX_EPOCH + Duration::from_secs(cached.expires);
            true
        } else {
            false
        }
    }

    pub fn download(&mut self, cache: &dyn ConfigurationCache) -> bool {
        let client = reqwest::blocking::Client::new();
        match client.get(&self.url).send() {
            Ok(resp) if resp.status().is_success() => {
                let etag = resp.headers()
                    .get("etag")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                let body = resp.text().unwrap_or_default();
                let data: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
                self.parse_config(&data);
                let cached = CachedConfiguration {
                    expires: self.expires.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
                    refresh_interval: 3600,
                    etag,
                    data,
                };
                cache.write_config_to_cache(&self.url, &cached);
                self.downloaded = true;
                true
            }
            Ok(resp) if resp.status().as_u16() == 304 => {
                self.downloaded = true;
                true
            }
            _ => false,
        }
    }

    fn parse_config(&mut self, data: &serde_json::Value) {
        if let Some(v) = data["MAXEVENTSIZEINBYTES"].as_i64() {
            self.max_event_size_in_bytes.set(v as i32);
        }
        if let Some(v) = data["MAXEVENTSPERPOST"].as_i64() {
            self.max_events_per_post.set(v as i32);
        }
        if let Some(v) = data["QUEUEDRAININTERVAL"].as_i64() {
            self.queue_drain_interval.set(v as i32);
        }
    }
}

pub struct ConfigurationManager {
    configurations: Vec<Configuration>,
    cache: Option<Box<dyn ConfigurationCache>>,
    update_callbacks: Vec<Box<dyn Fn() + Send>>,
}

impl ConfigurationManager {
    pub fn new() -> Self {
        ConfigurationManager {
            configurations: Vec::new(),
            cache: None,
            update_callbacks: Vec::new(),
        }
    }

    pub fn set_cache(&mut self, cache: Box<dyn ConfigurationCache>) {
        self.cache = Some(cache);
    }

    pub fn add(&mut self, config: Configuration) {
        self.configurations.push(config);
    }

    pub fn add_update_callback<F: Fn() + Send + 'static>(&mut self, cb: F) {
        self.update_callbacks.push(Box::new(cb));
    }

    pub fn load_cached_configs(&mut self) {
        if let Some(ref cache) = self.cache {
            for config in &mut self.configurations {
                config.load_from_cache(cache.as_ref());
            }
        }
    }

    pub fn download_configs(&mut self) {
        if let Some(ref cache) = self.cache {
            for config in &mut self.configurations {
                if config.needs_redownload() {
                    config.download(cache.as_ref());
                }
            }
        }
        for cb in &self.update_callbacks {
            cb();
        }
    }

    fn get_first_set<T: Clone>(&self, accessor: fn(&Configuration) -> &ConfigurationProperty<T>) -> Option<T> {
        for config in &self.configurations {
            if let Some(val) = accessor(config).get() {
                return Some(val.clone());
            }
        }
        None
    }

    pub fn get_max_event_size_in_bytes(&self) -> i32 {
        self.get_first_set(|c| &c.max_event_size_in_bytes).unwrap_or(6400)
    }

    pub fn get_max_events_per_post(&self) -> i32 {
        self.get_first_set(|c| &c.max_events_per_post).unwrap_or(500)
    }

    pub fn get_queue_drain_interval(&self) -> i32 {
        self.get_first_set(|c| &c.queue_drain_interval).unwrap_or(120)
    }
}

// Serde for CachedConfiguration (manual impl needed for HashMap)
use serde::{Deserialize, Serialize};
