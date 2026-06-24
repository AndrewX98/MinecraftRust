pub mod window;
pub mod input;
pub mod gl_context;

pub use window::GameWindow;
pub use input::GamepadState;
pub use gl_context::GLContext;

pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub resizable: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Minecraft".to_string(),
            width: 854,
            height: 480,
            fullscreen: false,
            resizable: true,
        }
    }
}

pub fn create_window(config: WindowConfig) -> Result<GameWindow, Box<dyn std::error::Error>> {
    GameWindow::new(config)
}
