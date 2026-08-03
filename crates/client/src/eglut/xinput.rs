pub struct XInputRuntime {
    pub xi2_available: bool,
}

pub static mut XINPUT_RT: Option<XInputRuntime> = None;
