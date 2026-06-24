use winit::event::WindowEvent;
use winit::event_loop::EventLoop;
use winit::window::Window;
use std::sync::{mpsc, Arc, Mutex};

use crate::WindowConfig;

#[derive(Clone)]
pub struct WindowHandle {
    tx: mpsc::Sender<WindowEvent>,
}

pub struct GameWindow {
    mode: Option<WindowMode>,
    config: WindowConfig,
    rx: Arc<Mutex<mpsc::Receiver<WindowEvent>>>,
    tx: mpsc::Sender<WindowEvent>,
}

enum WindowMode {
    Poll {
        window: Window,
        event_loop: EventLoop<()>,
    },
}

impl GameWindow {
    pub fn new(config: WindowConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let (tx, rx) = mpsc::channel();
        Ok(Self {
            mode: None,
            config,
            rx: Arc::new(Mutex::new(rx)),
            tx,
        })
    }

    /// Create window in non-blocking poll mode (for game loop integration)
    pub fn create_poll(&mut self) -> Result<WindowHandle, Box<dyn std::error::Error>> {
        let event_loop = EventLoop::new()?;
        let win_attrs = Window::default_attributes()
            .with_title(&self.config.title)
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.config.width as f64,
                self.config.height as f64,
            ))
            .with_resizable(self.config.resizable);

        let window = event_loop.create_window(win_attrs)?;

        log::info!("game-window: created window {}x{}", self.config.width, self.config.height);

        self.mode = Some(WindowMode::Poll {
            window,
            event_loop,
        });

        Ok(WindowHandle {
            tx: self.tx.clone(),
        })
    }

    /// Get a reference to the raw Window (for GL context creation)
    pub fn window(&self) -> Option<&Window> {
        match &self.mode {
            Some(WindowMode::Poll { window, .. }) => Some(window),
            None => None,
        }
    }

    /// Get a reference to the EventLoop (for GL context creation)
    pub fn event_loop(&self) -> Option<&EventLoop<()>> {
        match &self.mode {
            Some(WindowMode::Poll { event_loop, .. }) => Some(event_loop),
            None => None,
        }
    }

    /// Non-blocking event pump. Returns false if window was closed.
    pub fn poll_events(&mut self) -> bool {
        // For now, we just drain the channel
        if let Ok(rx) = self.rx.lock() {
            while let Ok(event) = rx.try_recv() {
                match event {
                    WindowEvent::CloseRequested => return false,
                    _ => {}
                }
            }
        }
        true
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let _event_loop: EventLoop<()> = EventLoop::new()?;
        // For standalone mode we'd need a proper event loop runner.
        // For now, create_poll is the primary entry point.
        todo!("Blocking run mode not yet implemented")
    }
}
