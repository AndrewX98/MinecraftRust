use chrono::{DateTime, Utc};

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct EventFlags: u32 {
        const PERSISTENCE_NORMAL = 1;
        const PERSISTENCE_CRITICAL = 2;
        const LATENCY_NORMAL = 256;
        const LATENCY_REALTIME = 512;
    }
}

#[derive(Debug, Clone)]
pub struct Event {
    pub name: String,
    pub data: serde_json::Value,
    pub flags: EventFlags,
    pub ids: Vec<String>,
    pub time: DateTime<Utc>,
}

impl Event {
    pub fn new(name: &str, data: serde_json::Value, flags: EventFlags) -> Self {
        Event {
            name: name.to_string(),
            data,
            flags,
            ids: Vec::new(),
            time: Utc::now(),
        }
    }

    pub fn with_ids(mut self, ids: Vec<String>) -> Self {
        self.ids = ids;
        self
    }

    pub fn with_time(mut self, time: DateTime<Utc>) -> Self {
        self.time = time;
        self
    }
}
