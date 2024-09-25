use bindings::{EngineFunctions, ENGINE_FUNCTIONS};
use console_hook::{hook_console_print, hook_write_console};
use parking_lot::Mutex;
use rcon::RconServer;
use rrplug::{mid::engine::WhichDll, prelude::*};
use std::{
    collections::HashMap,
    env,
    sync::mpsc::{self, Sender},
};

pub mod bindings;
pub mod console;
pub mod console_hook;
pub mod rcon;

const VALID_RCON_ARGS: [&str; 2] = ["rcon_ip_port", "rcon_password"];

pub struct RconPlugin {
    console_sender: Mutex<Sender<String>>,
    server: Option<Mutex<RconServer>>,
}

impl Plugin for RconPlugin {
    const PLUGIN_INFO: PluginInfo = PluginInfo::new(
        c"r2rcon-rs",
        c"R2RCON_RS",
        c"R2RCONRS",
        PluginContext::all(),
    );

    fn new(_reloaded: bool) -> Self {
        let (console_sender, console_recv) = mpsc::channel();

        let rcon_args = env::args()
            .zip(env::args().skip(1))
            .filter(|(cmd, _)| VALID_RCON_ARGS.contains(&&cmd[..]))
            .fold(HashMap::new(), |mut hash_map, (cmd, arg)| {
                _ = hash_map.insert(cmd, arg);
                hash_map
            });

        let mut server = None;

        'start_server: {
            let (Some(bind_ip), Some(password)) = (
                rcon_args.get(VALID_RCON_ARGS[0]),
                rcon_args.get(VALID_RCON_ARGS[1]),
            ) else {
                log::error!("the rcon args that were provided are invalid!");
                break 'start_server;
            };

            server = RconServer::try_new(bind_ip, password, console_recv)
                .map_err(|err| log::info!("failed to connect to socket : {err:?}"))
                .inspect(|_| {
                    hook_write_console();
                })
                .ok();
        }

        Self {
            console_sender: Mutex::new(console_sender),
            server: server.map(|s| s.into()),
        }
    }

    fn on_dll_load(&self, _: Option<&EngineData>, dll_ptr: &DLLPointer, _token: EngineToken) {
        unsafe { EngineFunctions::try_init(dll_ptr, &ENGINE_FUNCTIONS) };

        if let WhichDll::Client = dll_ptr.which_dll() {
            let addr = dll_ptr.get_dll_ptr() as isize;
            std::thread::spawn(move || _ = hook_console_print(addr));
        }
    }

    fn runframe(&self, _token: EngineToken) {
        _ = self.server.as_ref().map(|s| s.lock().run());
    }
}

entry!(RconPlugin);
