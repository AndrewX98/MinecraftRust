pub mod minecraft_version;
pub mod patch_utils;
pub mod hook;
pub mod hook_manager;
pub mod hybris_utils;
pub mod minecraft_utils;
pub mod mod_loader;

#[cfg(test)]
mod tests {
    #[test]
    fn bootstrap() {
        assert!(true);
    }
}