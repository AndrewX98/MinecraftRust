use chrono::Utc;
use crate::event::Event;

pub struct EventSerializer {
    i_key: Option<String>,
    app_id: Option<String>,
    app_ver: Option<String>,
    os: String,
    os_ver: String,
    epoch: i64,
    seq_num: i64,
    extensions: Vec<Box<dyn Extension + Send>>,
}

pub trait Extension {
    fn name(&self) -> &str;
    fn build(&self, event: &Event) -> serde_json::Value;
}

impl EventSerializer {
    pub fn new() -> Self {
        let (os, os_ver) = if cfg!(target_os = "linux") {
            ("Linux".into(), std::env::consts::ARCH.into())
        } else if cfg!(target_os = "macos") {
            ("macOS".into(), std::env::consts::ARCH.into())
        } else {
            ("Unknown".into(), String::new())
        };

        EventSerializer {
            i_key: None,
            app_id: None,
            app_ver: None,
            os,
            os_ver,
            epoch: Utc::now().timestamp(),
            seq_num: 0,
            extensions: Vec::new(),
        }
    }

    pub fn set_i_key(&mut self, key: &str) {
        self.i_key = Some(key.to_string());
    }

    pub fn set_app(&mut self, app_id: &str, app_ver: &str) {
        self.app_id = Some(app_id.to_string());
        self.app_ver = Some(app_ver.to_string());
    }

    pub fn add_extension(&mut self, ext: Box<dyn Extension + Send>) {
        self.extensions.push(ext);
    }

    pub fn create_envelope_for(&mut self, event: &Event) -> serde_json::Value {
        self.seq_num += 1;

        let mut ext = serde_json::Map::new();
        for e in &self.extensions {
            ext.insert(e.name().to_string(), e.build(event));
        }

        let mut envelope = serde_json::Map::new();
        envelope.insert("ver".into(), serde_json::Value::String("2.1".into()));
        envelope.insert("name".into(), serde_json::Value::String(event.name.clone()));
        envelope.insert("data".into(), event.data.clone());
        envelope.insert("time".into(), serde_json::Value::String(event.time.to_rfc3339()));
        envelope.insert("popSample".into(), serde_json::Value::from(100));
        envelope.insert("epoch".into(), serde_json::Value::from(self.epoch));
        envelope.insert("seqNum".into(), serde_json::Value::from(self.seq_num));

        if let Some(ref i_key) = self.i_key {
            envelope.insert("iKey".into(), serde_json::Value::String(format!("o:{}", i_key)));
        }

        let flags = event.flags.bits() as i64;
        envelope.insert("flags".into(), serde_json::Value::from(flags));
        envelope.insert("os".into(), serde_json::Value::String(self.os.clone()));
        envelope.insert("osVer".into(), serde_json::Value::String(self.os_ver.clone()));

        if let Some(ref app_id) = self.app_id {
            envelope.insert("appId".into(), serde_json::Value::String(app_id.clone()));
        }
        if let Some(ref app_ver) = self.app_ver {
            envelope.insert("appVer".into(), serde_json::Value::String(app_ver.clone()));
        }

        if !event.ids.is_empty() {
            envelope.insert("ids".into(), serde_json::Value::Array(
                event.ids.iter().map(|s| serde_json::Value::String(s.clone())).collect()
            ));
        }

        if !ext.is_empty() {
            envelope.insert("ext".into(), serde_json::Value::Object(ext));
        }

        serde_json::Value::Object(envelope)
    }
}

pub struct UserInfoExtension;
impl Extension for UserInfoExtension {
    fn name(&self) -> &str { "user" }
    fn build(&self, _event: &Event) -> serde_json::Value {
        serde_json::json!({"ver": "1.0"})
    }
}

pub struct OsInfoExtension;
impl Extension for OsInfoExtension {
    fn name(&self) -> &str { "os" }
    fn build(&self, _event: &Event) -> serde_json::Value {
        serde_json::json!({"ver": "1.0"})
    }
}

pub struct DeviceInfoExtension;
impl Extension for DeviceInfoExtension {
    fn name(&self) -> &str { "device" }
    fn build(&self, _event: &Event) -> serde_json::Value {
        serde_json::json!({"ver": "1.0"})
    }
}

pub struct AndroidExtension;
impl Extension for AndroidExtension {
    fn name(&self) -> &str { "android" }
    fn build(&self, event: &Event) -> serde_json::Value {
        serde_json::json!({
            "ver": "1.0",
            "libVer": "3.170921.0",
            "tickets": event.ids,
        })
    }
}
