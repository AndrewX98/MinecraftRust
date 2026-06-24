pub struct ErrorCodes;

impl ErrorCodes {
    pub const NO_SUCH_ACCOUNT: i32 = -100;
    pub const ACCOUNT_ALREADY_EXISTS: i32 = -101;
    pub const MUST_SHOW_UI: i32 = -102;
    pub const TOKEN_ACQUISITION_SERVER_ERROR: i32 = -110;
    pub const INTERNAL_ERROR: i32 = -200;
    pub const INTERNAL_UI_START_ERROR: i32 = -201;
    pub const OPERATION_CANCELLED: i32 = -501;
}
