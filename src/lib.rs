use bindings::{CmdSource, EngineFunctions, ENGINE_FUNCTIONS};
use console_hook::{hook_console_print, hook_write_console};
use parking_lot::Mutex;
use rcon::{RconServer, RconTask};
use rrplug::{mid::engine::WhichDll, prelude::*, to_c_string};
use std::{
    collections::HashMap,
    env,
    sync::mpsc::{self, Receiver, Sender},
};

pub mod bindings;
pub mod console_hook;
pub mod rcon;

const VALID_RCON_ARGS: [&str; 2] = ["rcon_ip_port", "rcon_password"];

pub struct RconPlugin {
    rcon_tasks: Mutex<Receiver<RconTask>>,
    rcon_send_tasks: Mutex<Sender<RconTask>>, // mutex is not needed but it must sync so clone on each thread
    console_sender: Mutex<Sender<String>>,
    console_recv: Mutex<Receiver<String>>,
    server: Option<Mutex<RconServer>>,
}

impl Plugin for RconPlugin {
    fn new(_: &PluginData) -> Self {
        let (sender, recv) = mpsc::channel();
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

            server = RconServer::try_new(&bind_ip, password)
                .map_err(|err| log::info!("failed to connect to socket : {err:?}"))
                .map(|s| {
                    hook_write_console();
                    s
                })
                .ok();
        }

        // std::thread::spawn(move || _ = run_rcon(args));

        Self {
            rcon_tasks: Mutex::new(recv),
            rcon_send_tasks: Mutex::new(sender),
            console_sender: Mutex::new(console_sender),
            console_recv: Mutex::new(console_recv),
            server: server.map(|s| s.into()),
        }
    }

    fn on_dll_load(&self, _: Option<&EngineData>, dll_ptr: &DLLPointer) {
        unsafe { EngineFunctions::try_init(dll_ptr, &ENGINE_FUNCTIONS) };

        if let WhichDll::Client = dll_ptr.which_dll() {
            let addr = dll_ptr.get_dll_ptr() as isize;
            std::thread::spawn(move || _ = hook_console_print(addr));
        }
    }

    fn runframe(&self) {
        // can be moved somewhere else

        let funcs = ENGINE_FUNCTIONS.wait();

        // if let Ok(task) = self.rcon_tasks.lock().try_recv() {
        if let Ok(tasks) = self
            .server
            .as_ref()
            // .map(|s| s.lock().run(self.console_recv.lock().try_recv().ok()))
            .map(|s| s.lock().run(None))
            .flatten()
            .ok_or(())
        {
            for task in tasks.into_iter() {
                match task {
                    RconTask::Runcommand(cmd) => unsafe {
                        log::info!("executing command : {cmd}");

                        let cmd = to_c_string!(cmd);
                        (funcs.cbuf_add_text_type)(
                            (funcs.cbuf_get_current_player)(),
                            cmd.as_ptr(),
                            CmdSource::Code,
                        );
                    },
                }
            }
        }
    }
}

fn run_rcon(server: &mut RconServer) -> std::convert::Infallible {
    let rcon = PLUGIN.wait();

    let rcon_send_tasks = rcon.rcon_send_tasks.lock();
    let console_recv = rcon.console_recv.lock();

    loop {
        let new_console_line = console_recv.try_recv().ok();

        if let Some(tasks) = server.run(new_console_line) {
            tasks
                .into_iter()
                .for_each(|task| rcon_send_tasks.send(task).expect("failed to send tasks"))
        }
    }
}

entry!(RconPlugin);
