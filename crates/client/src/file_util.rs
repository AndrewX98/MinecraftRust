use std::path::Path;

pub fn exists(path: &str) -> bool {
    Path::new(path).exists()
}

pub fn is_directory(path: &str) -> bool {
    Path::new(path).is_dir()
}

pub fn get_parent(path: &str) -> String {
    Path::new(path).parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default()
}

pub fn mkdir_recursive(path: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(path)
}

pub fn read_file(path: &str) -> std::io::Result<String> {
    std::fs::read_to_string(path)
}

pub fn read_file_bytes(path: &str) -> std::io::Result<Vec<u8>> {
    std::fs::read(path)
}
