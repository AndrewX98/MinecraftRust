use std::collections::VecDeque;
use std::fs;
use std::io::Write;
use std::path::Path;

pub trait BatchedEventList {
    fn data(&self) -> &[u8];
    fn data_size(&self) -> usize;
    fn event_count(&self) -> usize;
    fn has_more_events(&self) -> bool;
}

pub struct VectorBatchedEventList {
    data: Vec<u8>,
    events: usize,
    more: bool,
}

impl VectorBatchedEventList {
    pub fn new(data: Vec<u8>, events: usize, has_more: bool) -> Self {
        VectorBatchedEventList { data, events, more: has_more }
    }
}

impl BatchedEventList for VectorBatchedEventList {
    fn data(&self) -> &[u8] { &self.data }
    fn data_size(&self) -> usize { self.data.len() }
    fn event_count(&self) -> usize { self.events }
    fn has_more_events(&self) -> bool { self.more }
}

pub trait EventBatch: Send {
    fn add_event(&mut self, data: serde_json::Value) -> bool;
    fn get_events_for_upload(&mut self, max_count: usize, max_size: usize) -> Option<Box<dyn BatchedEventList>>;
    fn on_events_uploaded(&mut self, events: &dyn BatchedEventList);
    fn has_events(&self) -> bool;
}

pub struct MemoryEventBatch {
    events: VecDeque<serde_json::Value>,
    limit: usize,
}

impl MemoryEventBatch {
    pub fn new(limit: usize) -> Self {
        MemoryEventBatch {
            events: VecDeque::new(),
            limit,
        }
    }

    pub fn transfer_all_events(&mut self) -> Vec<serde_json::Value> {
        self.events.drain(..).collect()
    }
}

impl EventBatch for MemoryEventBatch {
    fn add_event(&mut self, data: serde_json::Value) -> bool {
        if self.events.len() >= self.limit {
            return false;
        }
        self.events.push_back(data);
        true
    }

    fn get_events_for_upload(&mut self, max_count: usize, _max_size: usize) -> Option<Box<dyn BatchedEventList>> {
        if self.events.is_empty() {
            return None;
        }
        let count = self.events.len().min(max_count);
        let lines: Vec<String> = self.events.drain(..count)
            .map(|v| serde_json::to_string(&v).unwrap_or_default())
            .collect();
        let data = lines.join("\n").into_bytes();
        Some(Box::new(VectorBatchedEventList::new(data, count, !self.events.is_empty())))
    }

    fn on_events_uploaded(&mut self, _events: &dyn BatchedEventList) {}

    fn has_events(&self) -> bool { !self.events.is_empty() }
}

pub struct FileEventBatch {
    path: String,
    max_count: usize,
    max_size: usize,
    finalized: bool,
}

impl FileEventBatch {
    pub fn new(path: &str) -> Self {
        FileEventBatch {
            path: path.to_string(),
            max_count: 500,
            max_size: 1024 * 1024,
            finalized: false,
        }
    }

    pub fn set_limit(&mut self, max_count: usize, max_size: usize) {
        self.max_count = max_count;
        self.max_size = max_size;
    }

    pub fn set_finalized(&mut self) {
        self.finalized = true;
    }

    fn read_events(&self) -> Vec<String> {
        if !Path::new(&self.path).exists() {
            return Vec::new();
        }
        let content = fs::read_to_string(&self.path).unwrap_or_default();
        content.lines().map(|l| l.to_string()).collect()
    }
}

impl EventBatch for FileEventBatch {
    fn add_event(&mut self, data: serde_json::Value) -> bool {
        if self.finalized {
            return false;
        }
        let line = serde_json::to_string(&data).unwrap_or_default();
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .unwrap();
        writeln!(file, "{}", line).ok();
        true
    }

    fn get_events_for_upload(&mut self, max_count: usize, _max_size: usize) -> Option<Box<dyn BatchedEventList>> {
        let events = self.read_events();
        if events.is_empty() {
            return None;
        }
        let count = events.len().min(max_count);
        let data = events[..count].join("\n").into_bytes();
        Some(Box::new(VectorBatchedEventList::new(data, count, events.len() > count)))
    }

    fn on_events_uploaded(&mut self, events: &dyn BatchedEventList) {
        let current = self.read_events();
        let remaining = current.len().saturating_sub(events.event_count());
        let data = current[current.len() - remaining..].join("\n");
        if remaining > 0 {
            fs::write(&self.path, &data).ok();
        } else {
            fs::remove_file(&self.path).ok();
        }
    }

    fn has_events(&self) -> bool { Path::new(&self.path).exists() }
}

pub struct MultiFileEventBatch {
    path: String,
    prefix: String,
    suffix: String,
    file_max_events: usize,
    file_max_size: usize,
}

impl MultiFileEventBatch {
    pub fn new(path: &str, prefix: &str, suffix: &str, file_max_events: usize, file_max_size: usize) -> Self {
        MultiFileEventBatch {
            path: path.to_string(),
            prefix: prefix.to_string(),
            suffix: suffix.to_string(),
            file_max_events,
            file_max_size,
        }
    }

    fn get_file_paths(&self) -> Vec<String> {
        let dir = Path::new(&self.path);
        if !dir.exists() {
            return Vec::new();
        }
        let mut files: Vec<String> = Vec::new();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(&self.prefix) && name.ends_with(&self.suffix) {
                    files.push(entry.path().to_string_lossy().to_string());
                }
            }
        }
        files.sort();
        files
    }

    fn get_active_file(&self) -> String {
        let files = self.get_file_paths();
        if files.is_empty() {
            format!("{}/{}{}", self.path, self.prefix, self.suffix)
        } else {
            files[files.len() - 1].clone()
        }
    }
}

impl EventBatch for MultiFileEventBatch {
    fn add_event(&mut self, data: serde_json::Value) -> bool {
        let active = self.get_active_file();
        let mut batch = FileEventBatch::new(&active);
        batch.set_limit(self.file_max_events, self.file_max_size);
        batch.add_event(data)
    }

    fn get_events_for_upload(&mut self, max_count: usize, max_size: usize) -> Option<Box<dyn BatchedEventList>> {
        let files = self.get_file_paths();
        if files.is_empty() {
            return None;
        }
        let mut all_data = Vec::new();
        let mut total_events = 0usize;
        for file in &files {
            let batch = FileEventBatch::new(file);
            let events = batch.read_events();
            if events.is_empty() {
                continue;
            }
            for event in &events {
                if total_events >= max_count || all_data.len() >= max_size {
                    return Some(Box::new(VectorBatchedEventList::new(all_data, total_events, true)));
                }
                if !all_data.is_empty() {
                    all_data.push(b'\n');
                }
                all_data.extend_from_slice(event.as_bytes());
                total_events += 1;
            }
        }
        if total_events == 0 {
            return None;
        }
        Some(Box::new(VectorBatchedEventList::new(all_data, total_events, false)))
    }

    fn on_events_uploaded(&mut self, events: &dyn BatchedEventList) {
        let mut remaining = events.event_count();
        let files = self.get_file_paths();
        for file in &files {
            if remaining == 0 {
                break;
            }
            let batch = FileEventBatch::new(file);
            let file_events = batch.read_events();
            if file_events.len() <= remaining {
                remaining -= file_events.len();
                fs::remove_file(file).ok();
            } else {
                let keep = file_events[remaining..].join("\n");
                remaining = 0;
                fs::write(file, &keep).ok();
            }
        }
    }

    fn has_events(&self) -> bool { !self.get_file_paths().is_empty() }
}

pub struct BufferedEventBatch {
    memory: MemoryEventBatch,
    wrapped: Box<dyn EventBatch>,
    #[allow(dead_code)]
    buffer_limit: usize,
}

impl BufferedEventBatch {
    pub fn new(wrapped: Box<dyn EventBatch>, buffer_limit: usize) -> Self {
        BufferedEventBatch {
            memory: MemoryEventBatch::new(buffer_limit),
            wrapped,
            buffer_limit,
        }
    }

    pub fn flush(&mut self) {
        let events = self.memory.transfer_all_events();
        for event in events {
            self.wrapped.add_event(event);
        }
    }
}

impl EventBatch for BufferedEventBatch {
    fn add_event(&mut self, data: serde_json::Value) -> bool {
        if self.memory.add_event(data.clone()) {
            return true;
        }
        self.flush();
        self.memory.add_event(data)
    }

    fn get_events_for_upload(&mut self, max_count: usize, max_size: usize) -> Option<Box<dyn BatchedEventList>> {
        self.flush();
        self.wrapped.get_events_for_upload(max_count, max_size)
    }

    fn on_events_uploaded(&mut self, events: &dyn BatchedEventList) {
        self.wrapped.on_events_uploaded(events);
    }

    fn has_events(&self) -> bool { self.memory.has_events() || self.wrapped.has_events() }
}
