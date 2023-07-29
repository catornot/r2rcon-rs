use rrplug::prelude::*;
use rrplug::{
    bindings::{command::CCommand, entity::CBaseClient},
    engine_functions,
};
use std::ffi::c_char;

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

engine_functions! {
    ENGINE_FUNCTIONS + EngineFunctions for PluginLoadDLL::ENGINE => {
        ccommand_tokenize = unsafe extern "C" fn(&mut Option<CCommand>, *const c_char, CmdSource) -> bool, at 0x418380;
        cbuf_add_text_type = unsafe extern "C" fn(EcommandTarget, *const c_char, CmdSource), at 0x1203B0;
        cbuf_execute = unsafe extern "C" fn(), at 0x1204B0;
        cbuf_get_current_player = unsafe extern "C" fn() -> EcommandTarget, at 0x120630;
        client_array = *const CBaseClient, at 0x12A53F90;
    }
}
