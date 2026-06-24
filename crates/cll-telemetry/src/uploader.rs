use std::time::Duration;

use crate::batch::{BatchedEventList, EventBatch};

#[derive(Debug, Clone)]
pub enum EventUploadStatus {
    Success,
    ErrorGeneric,
    ErrorRateLimit(Duration),
}

impl EventUploadStatus {
    pub fn is_success(&self) -> bool {
        matches!(self, EventUploadStatus::Success)
    }
}

pub struct EventUploadRequest<'a> {
    pub batch: &'a dyn BatchedEventList,
    pub headers: Vec<(String, String)>,
}

impl<'a> EventUploadRequest<'a> {
    pub fn new(batch: &'a dyn BatchedEventList) -> Self {
        EventUploadRequest {
            batch,
            headers: Vec::new(),
        }
    }
}

pub trait EventUploadStep {
    fn on_request(&self, _request: &mut EventUploadRequest) {}
    fn on_authentication_failed(&self) -> bool {
        false
    }
}

pub struct EventUploader {
    url: String,
    client: reqwest::blocking::Client,
    steps: Vec<Box<dyn EventUploadStep + Send + Sync>>,
}

impl EventUploader {
    pub fn new(url: &str) -> Self {
        EventUploader {
            url: url.to_string(),
            client: reqwest::blocking::Client::new(),
            steps: Vec::new(),
        }
    }

    pub fn add_step(&mut self, step: Box<dyn EventUploadStep + Send + Sync>) {
        self.steps.push(step);
    }

    pub fn send_events(
        &self,
        batch: &dyn BatchedEventList,
        compress: bool,
    ) -> EventUploadStatus {
        let mut request = EventUploadRequest::new(batch);
        for step in &self.steps {
            step.on_request(&mut request);
        }

        let data = batch.data();
        let body = if compress && data.len() > 1 {
            let compressed = crate::compressor::EventCompressor::compress(data);
            compressed
        } else {
            data.to_vec()
        };

        let mut req = self.client.post(&self.url)
            .header("Content-Type", "application/x-json-stream; charset=utf-8")
            .body(body);

        for (name, value) in &request.headers {
            req = req.header(name.as_str(), value.as_str());
        }

        match req.send() {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    EventUploadStatus::Success
                } else if status.as_u16() == 429 || status.as_u16() == 503 {
                    let retry_after = resp.headers()
                        .get("Retry-After")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok())
                        .map(Duration::from_secs)
                        .unwrap_or(Duration::from_secs(60));
                    EventUploadStatus::ErrorRateLimit(retry_after)
                } else if status.as_u16() == 401 {
                    let mut retried = false;
                    for step in &self.steps {
                        if step.on_authentication_failed() {
                            retried = true;
                            break;
                        }
                    }
                    if retried {
                        // Retry once
                        return self.send_events(batch, compress);
                    }
                    EventUploadStatus::ErrorGeneric
                } else {
                    EventUploadStatus::ErrorGeneric
                }
            }
            Err(_) => EventUploadStatus::ErrorGeneric,
        }
    }

    pub fn send_events_from_batch(
        &self,
        batch: &mut dyn EventBatch,
        max_count: usize,
        max_size: usize,
    ) -> EventUploadStatus {
        let events = batch.get_events_for_upload(max_count, max_size);
        match events {
            Some(events_ref) => {
                let status = self.send_events(events_ref.as_ref(), true);
                if status.is_success() {
                    batch.on_events_uploaded(events_ref.as_ref());
                }
                status
            }
            None => EventUploadStatus::Success,
        }
    }
}
