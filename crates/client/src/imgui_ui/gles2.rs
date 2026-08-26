//! GLES2 renderer for the imgui overlay — port of `imgui_impl_opengl3.cpp`
//! configured with `IMGUI_IMPL_OPENGL_ES2` (shaders `#version 100`), matching
//! what mcpelauncher-manifest uses. GL entry points resolve against the
//! system libGL already linked by `build.rs`; Mesa dispatches per-context,
//! so calling them with an ES2 context current works.

use dear_imgui_rs::{DrawCmd, DrawData, DrawVert, TextureId};
use std::ffi::{c_char, c_void};

type GLuint = u32;
type GLint = i32;
type GLsizei = i32;
type GLenum = u32;
type GLboolean = u8;
type GLsizeiptr = isize;

const GL_FALSE: u8 = 0;
const GL_TRUE: u8 = 1;
const GL_FLOAT: GLenum = 0x1406;
const GL_UNSIGNED_BYTE: GLenum = 0x1401;
const GL_UNSIGNED_SHORT: GLenum = 0x1403;
const GL_ARRAY_BUFFER: GLenum = 0x8892;
const GL_ELEMENT_ARRAY_BUFFER: GLenum = 0x8893;
const GL_STREAM_DRAW: GLenum = 0x88E0;
const GL_TEXTURE_2D: GLenum = 0x0DE1;
const GL_TEXTURE0: GLenum = 0x84C0;
const GL_ACTIVE_TEXTURE: GLenum = 0x84E0;
const GL_TEXTURE_BINDING_2D: GLenum = 0x8069;
const GL_BLEND: GLenum = 0x0BE2;
const GL_SCISSOR_TEST: GLenum = 0x0C11;
const GL_DEPTH_TEST: GLenum = 0x0B71;
const GL_CULL_FACE: GLenum = 0x0B44;
const GL_BLEND_SRC_RGB: GLenum = 0x80C9;
const GL_BLEND_DST_RGB: GLenum = 0x80C8;
const GL_BLEND_SRC_ALPHA: GLenum = 0x80CB;
const GL_BLEND_DST_ALPHA: GLenum = 0x80CA;
const GL_SRC_ALPHA: GLenum = 0x0302;
const GL_ONE_MINUS_SRC_ALPHA: GLenum = 0x0303;
const GL_TRIANGLES: GLenum = 0x0004;
const GL_VERTEX_SHADER: GLenum = 0x8B31;
const GL_FRAGMENT_SHADER: GLenum = 0x8B30;
const GL_COMPILE_STATUS: GLenum = 0x8B81;
const GL_LINK_STATUS: GLenum = 0x8B82;
const GL_VIEWPORT: GLenum = 0x0BA2;
const GL_SCISSOR_BOX: GLenum = 0x0C10;
const GL_CURRENT_PROGRAM: GLenum = 0x8B8D;
const GL_ARRAY_BUFFER_BINDING: GLenum = 0x8894;
const GL_ELEMENT_ARRAY_BUFFER_BINDING: GLenum = 0x8895;
const GL_LINEAR: GLint = 0x2601;
const GL_TEXTURE_MIN_FILTER: GLenum = 0x2801;
const GL_TEXTURE_MAG_FILTER: GLenum = 0x2800;
const GL_RGBA: GLenum = 0x1908;
const GL_UNPACK_ALIGNMENT: GLenum = 0x0CF5;

extern "C" {
    fn glActiveTexture(texture: GLenum);
    fn glAttachShader(program: GLuint, shader: GLuint);
    fn glBindBuffer(target: GLenum, buffer: GLuint);
    fn glBindTexture(target: GLenum, texture: GLuint);
    fn glBlendFunc(sfactor: GLenum, dfactor: GLenum);
    fn glBufferData(target: GLenum, size: GLsizeiptr, data: *const c_void, usage: GLenum);
    fn glBufferSubData(target: GLenum, offset: GLsizeiptr, size: GLsizeiptr, data: *const c_void);
    fn glCompileShader(shader: GLuint);
    fn glCreateProgram() -> GLuint;
    fn glCreateShader(shader_type: GLenum) -> GLuint;
    fn glDeleteShader(shader: GLuint);
    fn glDisable(cap: GLenum);
    fn glDrawElements(mode: GLenum, count: GLsizei, type_: GLenum, indices: *const c_void);
    fn glEnable(cap: GLenum);
    fn glEnableVertexAttribArray(index: GLuint);
    fn glGenBuffers(n: GLsizei, buffers: *mut GLuint);
    fn glGenTextures(n: GLsizei, textures: *mut GLuint);
    fn glDeleteTextures(n: GLsizei, textures: *const GLuint);
    fn glGetAttribLocation(program: GLuint, name: *const c_char) -> GLint;
    fn glGetUniformLocation(program: GLuint, name: *const c_char) -> GLint;
    fn glGetIntegerv(pname: GLenum, params: *mut GLint);
    fn glGetError() -> GLenum;
    fn glGetProgramiv(program: GLuint, pname: GLenum, params: *mut GLint);
    fn glGetShaderInfoLog(shader: GLuint, max_length: GLsizei, length: *mut GLsizei, info_log: *mut c_char);
    fn glGetShaderiv(shader: GLuint, pname: GLenum, params: *mut GLint);
    fn glIsEnabled(cap: GLenum) -> GLboolean;
    fn glLinkProgram(program: GLuint);
    fn glPixelStorei(pname: GLenum, param: GLint);
    fn glScissor(x: GLint, y: GLint, width: GLsizei, height: GLsizei);
    fn glShaderSource(shader: GLuint, count: GLsizei, string: *const *const c_char, length: *const GLint);
    fn glTexImage2D(
        target: GLenum, level: GLint, internal_format: GLint, width: GLsizei, height: GLsizei,
        border: GLint, format: GLenum, type_: GLenum, pixels: *const c_void,
    );
    fn glTexParameteri(target: GLenum, pname: GLenum, param: GLint);
    fn glUniform1i(location: GLint, v0: GLint);
    fn glUniformMatrix4fv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const f32);
    fn glUseProgram(program: GLuint);
    fn glVertexAttribPointer(index: GLuint, size: GLint, type_: GLenum, normalized: GLboolean, stride: GLsizei, pointer: *const c_void);
    fn glViewport(x: GLint, y: GLint, width: GLsizei, height: GLsizei);
}

// Same shaders as ImGui_ImplOpenGL3 with IMGUI_IMPL_OPENGL_ES2.
const VERTEX_SHADER: &str = concat!(
    "#version 100\n",
    "uniform mat4 ProjMtx;\n",
    "attribute vec2 Position;\n",
    "attribute vec2 UV;\n",
    "attribute vec4 Color;\n",
    "varying vec2 Frag_UV;\n",
    "varying vec4 Frag_Color;\n",
    "void main() {\n",
    "    Frag_UV = UV;\n",
    "    Frag_Color = Color;\n",
    "    gl_Position = ProjMtx * vec4(Position.xy, 0.0, 1.0);\n",
    "}\n"
);

const FRAGMENT_SHADER: &str = concat!(
    "#version 100\n",
    "precision mediump float;\n",
    "uniform sampler2D Texture;\n",
    "varying vec2 Frag_UV;\n",
    "varying vec4 Frag_Color;\n",
    "void main() {\n",
    "    gl_FragColor = Frag_Color * texture2D(Texture, Frag_UV);\n",
    "}\n"
);

pub struct Gles2Renderer {
    program: GLuint,
    loc_texture: GLint,
    loc_proj_mtx: GLint,
    attr_position: GLint,
    attr_uv: GLint,
    attr_color: GLint,
    vbo: GLuint,
    ibo: GLuint,
    vbo_size: usize,
    ibo_size: usize,
}

impl Gles2Renderer {
    pub unsafe fn new() -> Option<Self> {
        while glGetError() != 0 {}
        let vs = compile_shader(GL_VERTEX_SHADER, VERTEX_SHADER)?;
        let fs = compile_shader(GL_FRAGMENT_SHADER, FRAGMENT_SHADER)?;
        let program = glCreateProgram();
        if program == 0 {
            return None;
        }
        glAttachShader(program, vs);
        glAttachShader(program, fs);
        glLinkProgram(program);
        let mut ok = 0;
        glGetProgramiv(program, GL_LINK_STATUS, &mut ok);
        glDeleteShader(vs);
        glDeleteShader(fs);
        if ok == 0 {
            log::error!("ImGui GLES2: program link failed");
            return None;
        }
        let name = |s: &str| CStringPtr::new(s);
        let loc_texture = glGetUniformLocation(program, name("Texture").ptr());
        let loc_proj_mtx = glGetUniformLocation(program, name("ProjMtx").ptr());
        let attr_position = glGetAttribLocation(program, name("Position").ptr());
        let attr_uv = glGetAttribLocation(program, name("UV").ptr());
        let attr_color = glGetAttribLocation(program, name("Color").ptr());
        let mut vbo = 0;
        let mut ibo = 0;
        glGenBuffers(1, &mut vbo);
        glGenBuffers(1, &mut ibo);
        if vbo == 0 || ibo == 0 || attr_position < 0 || attr_uv < 0 || attr_color < 0 {
            log::error!("ImGui GLES2: buffer/attrib setup failed");
            return None;
        }
        Some(Gles2Renderer {
            program,
            loc_texture,
            loc_proj_mtx,
            attr_position,
            attr_uv,
            attr_color,
            vbo,
            ibo,
            vbo_size: 0,
            ibo_size: 0,
        })
    }

    /// Uploads RGBA font atlas pixels; returns the texture id to hand to imgui.
    pub unsafe fn upload_font_texture(&mut self, width: i32, height: i32, data: &[u8]) -> TextureId {
        let mut tex = 0;
        glGenTextures(1, &mut tex);
        glActiveTexture(GL_TEXTURE0);
        glBindTexture(GL_TEXTURE_2D, tex);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
        glTexImage2D(
            GL_TEXTURE_2D, 0, GL_RGBA as GLint, width, height, 0,
            GL_RGBA, GL_UNSIGNED_BYTE, data.as_ptr() as *const c_void,
        );
        TextureId::new(tex as u64)
    }

    pub unsafe fn delete_texture(&mut self, id: TextureId) {
        let tex = id.id() as GLuint;
        glDeleteTextures(1, &tex);
    }
    pub unsafe fn render(&mut self, draw_data: &DrawData) {
        let fb_scale = draw_data.framebuffer_scale();
        let display_size = draw_data.display_size();
        if display_size[0] <= 0.0 || display_size[1] <= 0.0 || fb_scale[0] <= 0.0 || fb_scale[1] <= 0.0 {
            return;
        }
        let fb_width = (display_size[0] * fb_scale[0]) as GLsizei;
        let fb_height = (display_size[1] * fb_scale[1]) as GLsizei;

        // Save GL state (ES2 subset of imgui_impl_opengl3's restore list).
        let mut last_active_texture = 0;
        glGetIntegerv(GL_ACTIVE_TEXTURE, &mut last_active_texture);
        glActiveTexture(GL_TEXTURE0);
        let mut last_texture = 0;
        glGetIntegerv(GL_TEXTURE_BINDING_2D, &mut last_texture);
        let mut last_program = 0;
        glGetIntegerv(GL_CURRENT_PROGRAM, &mut last_program);
        let mut last_array_buffer = 0;
        glGetIntegerv(GL_ARRAY_BUFFER_BINDING, &mut last_array_buffer);
        let mut last_element_array_buffer = 0;
        glGetIntegerv(GL_ELEMENT_ARRAY_BUFFER_BINDING, &mut last_element_array_buffer);
        let (last_blend_src_rgb, last_blend_dst_rgb, last_blend_src_alpha, last_blend_dst_alpha) =
            get_blend_func();
        let last_enable_blend = glIsEnabled(GL_BLEND);
        let last_enable_scissor_test = glIsEnabled(GL_SCISSOR_TEST);
        let last_enable_depth_test = glIsEnabled(GL_DEPTH_TEST);
        let last_enable_cull_face = glIsEnabled(GL_CULL_FACE);
        let mut last_viewport = [0i32; 4];
        glGetIntegerv(GL_VIEWPORT, last_viewport.as_mut_ptr());
        let mut last_scissor_box = [0i32; 4];
        glGetIntegerv(GL_SCISSOR_BOX, last_scissor_box.as_mut_ptr());

        // Setup render state: alpha blending, no depth/cull/scissor.
        glActiveTexture(GL_TEXTURE0);
        glEnable(GL_BLEND);
        glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
        glDisable(GL_DEPTH_TEST);
        glDisable(GL_CULL_FACE);
        glDisable(GL_SCISSOR_TEST);
        glViewport(0, 0, fb_width, fb_height);

        // Orthographic projection, top-left origin (column-major).
        let w = display_size[0];
        let h = display_size[1];
        let proj_mtx: [f32; 16] = [
            2.0 / w, 0.0, 0.0, 0.0,
            0.0, -2.0 / h, 0.0, 0.0,
            0.0, 0.0, -1.0, 0.0,
            -1.0, 1.0, 0.0, 1.0,
        ];
        glUseProgram(self.program);
        glUniform1i(self.loc_texture, 0);
        glUniformMatrix4fv(self.loc_proj_mtx, 1, GL_FALSE, proj_mtx.as_ptr());

        const VERT_STRIDE: usize = std::mem::size_of::<DrawVert>(); // pos+uv+col = 20 bytes
        let display_pos = draw_data.display_pos();

        for draw_list in draw_data.draw_lists() {
            let vtx = draw_list.vtx_buffer();
            let idx = draw_list.idx_buffer();
            if vtx.is_empty() || idx.is_empty() {
                continue;
            }
            glBindBuffer(GL_ARRAY_BUFFER, self.vbo);
            self.upload_vertices(GL_ARRAY_BUFFER, vtx.len() * VERT_STRIDE, vtx.as_ptr() as *const c_void);
            glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, self.ibo);
            self.upload_vertices(GL_ELEMENT_ARRAY_BUFFER, idx.len() * std::mem::size_of::<u16>(), idx.as_ptr() as *const c_void);

            glVertexAttribPointer(self.attr_position as GLuint, 2, GL_FLOAT, GL_FALSE, VERT_STRIDE as GLsizei, std::ptr::null());
            glVertexAttribPointer(self.attr_uv as GLuint, 2, GL_FLOAT, GL_FALSE, VERT_STRIDE as GLsizei, 8 as *const c_void);
            // Colors are packed little-endian RGBA bytes in ImDrawVert.
            glVertexAttribPointer(self.attr_color as GLuint, 4, GL_UNSIGNED_BYTE, GL_TRUE, VERT_STRIDE as GLsizei, 16 as *const c_void);
            glEnableVertexAttribArray(self.attr_position as GLuint);
            glEnableVertexAttribArray(self.attr_uv as GLuint);
            glEnableVertexAttribArray(self.attr_color as GLuint);

            for cmd in draw_list.commands() {
                let (count, clip, tex_id, idx_off) = match cmd {
                    DrawCmd::Elements { count, cmd_params } => {
                        (count, cmd_params.clip_rect, cmd_params.texture_id, cmd_params.idx_offset)
                    }
                    // 1.92 sampler hints: our single font texture is LINEAR already.
                    DrawCmd::SetSamplerLinear | DrawCmd::SetSamplerNearest => continue,
                    DrawCmd::ResetRenderState | DrawCmd::RawCallback(_) => continue,
                };
                let cx = ((clip[0] - display_pos[0]) * fb_scale[0]) as GLint;
                let cy = ((clip[1] - display_pos[1]) * fb_scale[1]) as GLint;
                let cw = ((clip[2] - clip[0]) * fb_scale[0]) as GLsizei;
                let ch = ((clip[3] - clip[1]) * fb_scale[1]) as GLsizei;
                if cx + cw <= 0 || cy + ch <= 0 || cx >= fb_width || cy >= fb_height {
                    continue;
                }
                glBindTexture(GL_TEXTURE_2D, tex_id.id() as GLuint);
                glScissor(cx, fb_height - (cy + ch), cw, ch);
                glDrawElements(
                    GL_TRIANGLES,
                    count as GLsizei,
                    GL_UNSIGNED_SHORT,
                    (idx_off * std::mem::size_of::<u16>()) as *const c_void,
                );
            }
        }

        // Restore modified GL state.
        glUseProgram(last_program as GLuint);
        glBindTexture(GL_TEXTURE_2D, last_texture as GLuint);
        glActiveTexture(last_active_texture as GLenum);
        glBlendFuncSeparate(last_blend_src_rgb, last_blend_dst_rgb, last_blend_src_alpha, last_blend_dst_alpha);
        set_enabled(GL_BLEND, last_enable_blend);
        set_enabled(GL_DEPTH_TEST, last_enable_depth_test);
        set_enabled(GL_CULL_FACE, last_enable_cull_face);
        set_enabled(GL_SCISSOR_TEST, last_enable_scissor_test);
        glViewport(
            last_viewport[0], last_viewport[1], last_viewport[2], last_viewport[3],
        );
        glScissor(
            last_scissor_box[0], last_scissor_box[1], last_scissor_box[2], last_scissor_box[3],
        );
        glBindBuffer(GL_ARRAY_BUFFER, last_array_buffer as GLuint);
        glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, last_element_array_buffer as GLuint);

        while glGetError() != 0 {}
    }

    /// Orphan-then-fill, matching ImGui_ImplOpenGL3's buffer update strategy.
    unsafe fn upload_vertices(&mut self, target: GLenum, size: usize, data: *const c_void) {
        let known = match target {
            GL_ARRAY_BUFFER => &mut self.vbo_size,
            _ => &mut self.ibo_size,
        };
        if *known < size {
            *known = size * 2;
            glBufferData(target, *known as GLsizeiptr, std::ptr::null(), GL_STREAM_DRAW);
        }
        glBufferSubData(target, 0, size as GLsizeiptr, data);
    }
}

unsafe fn compile_shader(kind: GLenum, source: &str) -> Option<GLuint> {
    let shader = glCreateShader(kind);
    if shader == 0 {
        return None;
    }
    let src = CStringPtr::new(source);
    let ptr = [src.ptr()];
    glShaderSource(shader, 1, ptr.as_ptr(), std::ptr::null());
    glCompileShader(shader);
    let mut ok = 0;
    glGetShaderiv(shader, GL_COMPILE_STATUS, &mut ok);
    if ok == 0 {
        let mut len = 0;
        glGetShaderInfoLog(shader, 0, &mut len, std::ptr::null_mut());
        let mut buf = vec![0u8; len.max(1) as usize];
        glGetShaderInfoLog(shader, len, std::ptr::null_mut(), buf.as_mut_ptr() as *mut c_char);
        log::error!("ImGui GLES2 shader compile failed: {}", String::from_utf8_lossy(&buf));
        glDeleteShader(shader);
        return None;
    }
    Some(shader)
}

#[inline]
unsafe fn get_blend_func() -> (GLenum, GLenum, GLenum, GLenum) {
    let mut v = [0i32; 4];
    glGetIntegerv(GL_BLEND_SRC_RGB, &mut v[0]);
    glGetIntegerv(GL_BLEND_DST_RGB, &mut v[1]);
    glGetIntegerv(GL_BLEND_SRC_ALPHA, &mut v[2]);
    glGetIntegerv(GL_BLEND_DST_ALPHA, &mut v[3]);
    (v[0] as GLenum, v[1] as GLenum, v[2] as GLenum, v[3] as GLenum)
}

#[inline]
unsafe fn set_enabled(cap: GLenum, was_enabled: GLboolean) {
    if was_enabled != 0 {
        glEnable(cap);
    } else {
        glDisable(cap);
    }
}

extern "C" {
    #[link_name = "glBlendFuncSeparate"]
    fn glBlendFuncSeparate(src_rgb: GLenum, dst_rgb: GLenum, src_alpha: GLenum, dst_alpha: GLenum);
}

/// NUL-terminated string with a stable pointer for FFI calls.
struct CStringPtr(Vec<u8>);
impl CStringPtr {
    fn new(s: &str) -> Self {
        let mut v = Vec::with_capacity(s.len() + 1);
        v.extend_from_slice(s.as_bytes());
        v.push(0);
        CStringPtr(v)
    }
    fn ptr(&self) -> *const c_char {
        self.0.as_ptr() as *const c_char
    }
}
