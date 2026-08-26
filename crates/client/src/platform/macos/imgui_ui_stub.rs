//! macOS stub for the imgui overlay (Linux-only GLES2 renderer + eglut
//! hooks). Mirrors the `imgui_ui::mod` API so callers compile unchanged;
//! see docs/PORT_MACOS.md for the Cocoa/GLFW port plan.

pub mod gles2 {
    use dear_imgui_rs::{DrawData, TextureId};

    pub struct Gles2Renderer;

    impl Gles2Renderer {
        pub unsafe fn new() -> Option<Self> {
            None
        }
        pub unsafe fn upload_font_texture(&mut self, _w: i32, _h: i32, _data: &[u8]) -> TextureId {
            TextureId::new(0)
        }
        pub unsafe fn delete_texture(&mut self, _id: TextureId) {}
        pub unsafe fn render(&mut self, _draw_data: &DrawData) {}
    }
}

pub fn init_once() {}

pub fn draw_frame() {}

pub fn record_mouse_pos(_x: f64, _y: f64) {}

pub fn record_mouse_button(_xbtn: i32, _down: bool) {}

pub fn record_mouse_wheel(_dx: f64, _dy: f64) {}

pub fn record_key(_vk: i32, _down: bool) {}

pub fn record_text(_bytes: &[u8]) {}

pub fn want_capture_mouse() -> bool {
    false
}

pub fn want_capture_keyboard() -> bool {
    false
}
