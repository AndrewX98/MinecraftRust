use std::ffi::CString;
use std::num::NonZeroU32;

use glutin::config::{Api, ConfigTemplateBuilder};
use glutin::context::{ContextApi, ContextAttributesBuilder, PossiblyCurrentContext};
use glutin::display::{Display, GetGlDisplay};
use glutin::prelude::{GlDisplay, GlSurface, NotCurrentGlContext, PossiblyCurrentGlContext};
use glutin::surface::{Surface, SurfaceAttributesBuilder, WindowSurface};
use glutin_winit::DisplayBuilder;
use raw_window_handle::HasWindowHandle;
use winit::event_loop::EventLoop;
use winit::window::Window;

pub struct GLContext {
    context: PossiblyCurrentContext,
    surface: Surface<WindowSurface>,
    display: Display,
}

impl GLContext {
    pub fn new(
        window: &Window,
        event_loop: &EventLoop<()>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (window_opt, gl_config) = DisplayBuilder::new()
            .with_preference(glutin_winit::ApiPreference::PreferEgl)
            .build(event_loop, ConfigTemplateBuilder::new().with_api(Api::GLES2 | Api::GLES3), |mut configs| {
                configs.next().expect("No OpenGL ES config found")
            })?;

        if window_opt.is_some() {
            // The DisplayBuilder already created a window for us, but we already have one
            log::warn!("GLContext: DisplayBuilder created a window but we already had one");
        }

        let display = gl_config.display();

        let raw_window_handle = window.window_handle()?.as_raw();

        let context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(None))
            .build(Some(raw_window_handle));

        let not_current_context = unsafe {
            display.create_context(&gl_config, &context_attributes)?
        };

        let size = window.inner_size();
        let w = NonZeroU32::new(size.width).expect("window width must be non-zero");
        let h = NonZeroU32::new(size.height).expect("window height must be non-zero");

        let attrs = unsafe {
            SurfaceAttributesBuilder::<WindowSurface>::new()
                .build(raw_window_handle, w, h)
        };
        let surface = unsafe {
            display.create_window_surface(&gl_config, &attrs)?
        };

        let context = not_current_context.make_current(&surface)?;

        log::info!("GLContext: created OpenGL ES context, size={}x{}", w.get(), h.get());

        Ok(Self {
            context,
            surface,
            display,
        })
    }

    pub fn make_current(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.context.make_current(&self.surface)?;
        Ok(())
    }

    pub fn swap_buffers(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.surface.swap_buffers(&self.context)?;
        Ok(())
    }

    pub fn get_proc_address(&self, symbol: &str) -> *const std::ffi::c_void {
        let cstr = CString::new(symbol).unwrap_or_default();
        self.display.get_proc_address(&cstr)
    }

    pub fn get_size(&self) -> (u32, u32) {
        let w = self.surface.width().unwrap_or(0);
        let h = self.surface.height().unwrap_or(0);
        (w, h)
    }
}
