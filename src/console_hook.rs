use crate::{
    bindings::{CGameConsole, CreateInterface, IConsoleDisplayFunc},
    exports::PLUGIN,
};
use retour::static_detour;
use std::{
    ffi::{c_char, c_void, CStr},
    mem::transmute,
};
use windows_sys::Win32::{
    Foundation::{BOOL, HANDLE},
    System::LibraryLoader::{FreeLibrary, GetModuleHandleA, GetProcAddress},
};

// static HookWriteConsoleA: GenericDetour<>

static_detour! {
    static HookWriteConsoleA: unsafe extern "system" fn(HANDLE, *const c_void, u32, *mut u32, *const c_void) -> BOOL;
    static HookPrint: unsafe extern "C" fn(*const IConsoleDisplayFunc, *const c_char);
}

pub fn hook_write_console() {
    unsafe {
        if !std::env::args().any(|arg| arg == "-dedicated") {
            log::info!("this isn't a dedicated server; not hooking WriteConsoleA");
            return;
        }

        let kernel32 = GetModuleHandleA("kernel32.dll\0".as_ptr());
        let write_console = transmute(GetProcAddress(kernel32, "WriteConsoleA\0".as_ptr()));

        if let Err(err) = HookWriteConsoleA.initialize(write_console, write_console_hook) {
            log::error!("couldn't hook WriteConsoleA: {err}");
        } else {
            log::info!("hooked WriteConsoleA!");
        }
        _ = HookWriteConsoleA.enable();

        FreeLibrary(kernel32);
    }
}

pub fn hook_console_print(addr: isize) -> Option<()> {
    unsafe {
        if PLUGIN.wait().console_recv.try_lock().is_some() {
            log::warn!("rcon not running -> no Print hook");
            return None;
        }

        // let addr = GetModuleHandleA("client.dll\0".as_ptr());
        let create_interface: CreateInterface =
            match GetProcAddress(addr, "CreateInterface\0".as_ptr()) {
                Some(f) => transmute(f),
                None => return Some(log::error!("couldn't get CreateInterface")),
            };
        let cgame_console: &CGameConsole =
            match create_interface("GameConsole004\0".as_ptr() as *const i8, std::ptr::null())
                .as_ref()
            {
                Some(c) => transmute(c),
                None => return Some(log::error!("couldn't get GameConsole004")),
            };

        #[allow(clippy::while_immutable_condition)] // edited by other threads
        while !cgame_console.initialized {} // unsound access

        let display_func = &log_if_null(
            log_if_null(cgame_console.console, "CConsoleDialog")?.console_panel,
            "CConsolePanel",
        )?
        .iconsole_display_func;
        let print = log_if_null(display_func.vtable, "IConsoleDisplayFuncVtable")?.print;

        if let Err(err) = HookPrint.initialize(print, print_hook) {
            log::error!("couldn't hook Print: {err}");
        } else {
            log::info!("hooked Print!");
        }
        _ = HookPrint.enable();

        Some(())
    }
}

unsafe fn log_if_null<'a, T: std::fmt::Debug>(ptr: *const T, msg: &str) -> Option<&'a T> {
    match ptr.as_ref() {
        Some(t) => Some(t),
        None => {
            log::error!("{msg} is null");
            None
        }
    }
}

fn print_hook(this: *const IConsoleDisplayFunc, message: *const c_char) {
    let line = unsafe { CStr::from_ptr(message).to_string_lossy().to_string() };
    let str_line: &str = &line;

    match str_line {
        " " => {}
        "] " => {}
        "\n" => {}
        _ => {
            if let Some(plugin) = PLUGIN.get() {
                _ = plugin.console_sender.lock().send(line);
            }
        }
    }

    unsafe { HookPrint.call(this, message) };
}

fn write_console_hook(
    hconsoleoutput: HANDLE,
    lpbuffer: *const c_void,
    nnumberofcharstowrite: u32,
    lpnumberofcharswritten: *mut u32,
    lpreserved: *const c_void,
) -> BOOL {
    let buffer = unsafe {
        std::slice::from_raw_parts(
            transmute::<_, *const u8>(lpbuffer),
            nnumberofcharstowrite as usize,
        )
    };

    let raw_cmd_output = String::from_utf8_lossy(buffer);

    let cmd_output = raw_cmd_output
        .split("]\u{1b}[39;49m ")
        .last()
        .unwrap_or_default();

    if let Some(plugin) = PLUGIN.get() {
        _ = plugin.console_sender.lock().send(cmd_output.to_string());
    }

    unsafe {
        HookWriteConsoleA.call(
            hconsoleoutput,
            lpbuffer,
            nnumberofcharstowrite,
            lpnumberofcharswritten,
            lpreserved,
        )
    }
}
