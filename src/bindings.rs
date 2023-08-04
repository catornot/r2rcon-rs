use rrplug::{
    bindings::{command::CCommand, entity::CBaseClient},
    engine_functions,
};
use std::ffi::{c_char, c_int, c_uchar, c_void};

pub type CreateInterface = unsafe extern "C" fn(*const c_char, *const c_int) -> *const c_void;

#[derive(Debug, Clone)]
#[repr(C)]
pub enum CmdSource {
    // Added to the console buffer by gameplay code.  Generally unrestricted.
    Code,

    // Sent from code via engine->ClientCmd, which is restricted to commands visible
    // via FCVAR_GAMEDLL_FOR_REMOTE_CLIENTS.
    ClientCmd,

    // Typed in at the console or via a user key-bind.  Generally unrestricted, although
    // the client will throttle commands sent to the server this way to 16 per second.
    UserInput,

    // Came in over a net connection as a clc_stringcmd
    // host_client will be valid during this state.
    //
    // Restricted to FCVAR_GAMEDLL commands (but not convars) and special non-ConCommand
    // server commands hardcoded into gameplay code (e.g. "joingame")
    NetClient,

    // Received from the server as the client
    //
    // Restricted to commands with FCVAR_SERVER_CAN_EXECUTE
    NetServer,

    // Being played back from a demo file
    //
    // Not currently restricted by convar flag, but some commands manually ignore calls
    // from this source.  FIXME: Should be heavily restricted as demo commands can come
    // from untrusted sources.
    DemoFile,

    // Invalid value used when cleared
    Invalid = -1,
}

#[derive(Debug, Clone)]
#[repr(C)]
pub enum EcommandTarget {
    CbufFirstPlayer = 0,
    CbufLastPlayer = 1,
    CbufServer = 2,

    CbufCount,
}

#[repr(C)]
#[derive(Debug)]
pub struct CGameConsole {
    pub vtable: *const c_void,
    pub initialized: bool,
    pub console: *const CConsoleDialog,
}

#[repr(C)]
#[derive(Debug)]
pub struct CConsoleDialog {
    pub vtable: *const c_void,
    pub unk: [c_uchar; 0x398],
    pub console_panel: *const CConsolePanel,
}

#[repr(C)]
#[derive(Debug)]
pub struct CConsolePanel {
    pub editable_panel: EditablePanel,
    pub iconsole_display_func: IConsoleDisplayFunc,
}

#[repr(C)]
#[derive(Debug)]
pub struct EditablePanel {
    pub vtable_editable_panel: *const c_void,
    pub unk: [c_uchar; 0x2B0],
}

#[repr(C)]
#[derive(Debug)]
pub struct IConsoleDisplayFunc {
    pub vtable: *const IConsoleDisplayFuncVtable,
}

#[repr(C)]
#[derive(Debug)]
pub struct IConsoleDisplayFuncVtable {
    pub color_print: *const c_void,
    pub print: unsafe extern "C" fn(this: *const IConsoleDisplayFunc, message: *const c_char),
    pub dprint: unsafe extern "C" fn(this: *const IConsoleDisplayFunc, message: *const c_char),
}

engine_functions! {
    ENGINE_FUNCTIONS + EngineFunctions for WhichDll::Engine => {
        ccommand_tokenize = unsafe extern "C" fn(&mut Option<CCommand>, *const c_char, CmdSource) -> bool, at 0x418380;
        cbuf_add_text_type = unsafe extern "C" fn(EcommandTarget, *const c_char, CmdSource), at 0x1203B0;
        cbuf_execute = unsafe extern "C" fn(), at 0x1204B0;
        cbuf_get_current_player = unsafe extern "C" fn() -> EcommandTarget, at 0x120630;
        client_array = *const CBaseClient, at 0x12A53F90;
    }
}
