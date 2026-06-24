use std::ffi::{c_char, c_int, c_uint, c_void};
use x11::xlib::Display;

pub type EGLDisplay = *mut c_void;
pub type EGLConfig = *mut c_void;
pub type EGLContext = *mut c_void;
pub type EGLSurface = *mut c_void;
pub type EGLNativeWindowType = u64;
pub type EGLint = c_int;
pub type EGLenum = c_uint;
pub type EGLBoolean = c_int;

pub const EGL_TRUE: EGLBoolean = 1;
pub const EGL_FALSE: EGLBoolean = 0;
pub const EGL_NONE: EGLint = 0x3038;
pub const EGL_NO_SURFACE: EGLSurface = std::ptr::null_mut();
pub const EGL_NO_CONTEXT: EGLContext = std::ptr::null_mut();
pub const EGL_NO_DISPLAY: EGLDisplay = std::ptr::null_mut();
pub const EGL_DEFAULT_DISPLAY: *mut Display = std::ptr::null_mut();
pub const EGL_WINDOW_BIT: EGLint = 0x0004;
pub const EGL_OPENGL_ES_BIT: EGLint = 0x0001;
pub const EGL_OPENGL_ES2_BIT: EGLint = 0x0004;
pub const EGL_OPENGL_BIT: EGLint = 0x0008;
pub const EGL_OPENVG_BIT: EGLint = 0x0010;
pub const EGL_RED_SIZE: EGLint = 0x3024;
pub const EGL_GREEN_SIZE: EGLint = 0x3023;
pub const EGL_BLUE_SIZE: EGLint = 0x3022;
pub const EGL_ALPHA_SIZE: EGLint = 0x3021;
pub const EGL_DEPTH_SIZE: EGLint = 0x3025;
pub const EGL_STENCIL_SIZE: EGLint = 0x3025;
pub const EGL_SURFACE_TYPE: EGLint = 0x3033;
pub const EGL_RENDERABLE_TYPE: EGLint = 0x3040;
pub const EGL_NATIVE_VISUAL_ID: EGLint = 0x302E;
pub const EGL_CONTEXT_CLIENT_VERSION: EGLint = 0x3098;
pub const EGL_OPENGL_ES_API: EGLenum = 0x30A0;
pub const EGL_OPENGL_API: EGLenum = 0x30A2;
pub const EGL_OPENVG_API: EGLenum = 0x30A3;

extern "C" {
    pub fn eglGetDisplay(native_dpy: *mut Display) -> EGLDisplay;
    pub fn eglInitialize(dpy: EGLDisplay, major: *mut EGLint, minor: *mut EGLint) -> EGLBoolean;
    pub fn eglTerminate(dpy: EGLDisplay) -> EGLBoolean;
    pub fn eglQueryString(dpy: EGLDisplay, name: EGLint) -> *const c_char;
    pub fn eglChooseConfig(dpy: EGLDisplay, attrib_list: *const EGLint, configs: *mut EGLConfig,
                           config_size: EGLint, num_config: *mut EGLint) -> EGLBoolean;
    pub fn eglGetConfigAttrib(dpy: EGLDisplay, config: EGLConfig, attribute: EGLint, value: *mut EGLint) -> EGLBoolean;
    pub fn eglBindAPI(api: EGLenum) -> EGLBoolean;
    pub fn eglCreateContext(dpy: EGLDisplay, config: EGLConfig, share_context: EGLContext,
                            attrib_list: *const EGLint) -> EGLContext;
    pub fn eglDestroyContext(dpy: EGLDisplay, ctx: EGLContext) -> EGLBoolean;
    pub fn eglCreateWindowSurface(dpy: EGLDisplay, config: EGLConfig, win: EGLNativeWindowType,
                                  attrib_list: *const EGLint) -> EGLSurface;
    pub fn eglDestroySurface(dpy: EGLDisplay, surface: EGLSurface) -> EGLBoolean;
    pub fn eglMakeCurrent(dpy: EGLDisplay, draw: EGLSurface, read: EGLSurface, ctx: EGLContext) -> EGLBoolean;
    pub fn eglSwapBuffers(dpy: EGLDisplay, surface: EGLSurface) -> EGLBoolean;
    pub fn eglSwapInterval(dpy: EGLDisplay, interval: EGLint) -> EGLBoolean;
    pub fn setlocale(category: c_int, locale: *const c_char) -> *mut c_char;
}
