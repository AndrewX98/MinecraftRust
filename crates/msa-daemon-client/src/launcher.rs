use daemon_utils::daemon_launcher::DaemonLauncher;
use util::file_util::EnvPathUtil;

pub struct ServiceLauncher {
    executable_path: String,
    data_path: String,
    service_path: String,
}

impl ServiceLauncher {
    pub fn new(executable_path: &str, data_path: Option<&str>) -> Self {
        let data_path = data_path
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{}/msa", EnvPathUtil::get_data_home()));
        let service_path = format!("{}/msa-daemon-ipc.sock", data_path);
        ServiceLauncher {
            executable_path: executable_path.to_string(),
            data_path,
            service_path,
        }
    }
}

impl DaemonLauncher for ServiceLauncher {
    fn service_path(&self) -> &str {
        &self.service_path
    }

    fn get_arguments(&self) -> Vec<String> {
        vec![
            self.executable_path.clone(),
            "-d".into(),
            self.data_path.clone(),
            "-x".into(),
        ]
    }
}
