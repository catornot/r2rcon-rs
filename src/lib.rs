use bindings::{CmdSource, EngineFunctions, ENGINE_FUNCTIONS};
use console_hook::{hook_console_print, hook_write_console};
use parking_lot::Mutex;
use rcon::{RconServer, RconTask};
use rrplug::{
    bindings::class_types::client::CClient, mid::engine::WhichDll, prelude::*, to_c_string,
};
use std::{
    collections::HashMap,
    env,
    ffi::c_void,
    sync::mpsc::{self, Receiver, Sender},
};

pub mod bindings;
pub mod console_hook;
pub mod rcon;

const VALID_RCON_ARGS: [&str; 2] = ["rcon_ip_port", "rcon_password"];

#[derive(Debug)]
pub struct RconPlugin {
    rcon_tasks: Mutex<Receiver<RconTask>>,
    rcon_send_tasks: Mutex<Sender<RconTask>>, // mutex is not needed but it must sync so clone on each thread
    console_sender: Mutex<Sender<String>>,
    console_recv: Mutex<Receiver<String>>,
}

impl Plugin for RconPlugin {
    fn new(_: &PluginData) -> Self {
        let (sender, recv) = mpsc::channel();
        let (console_sender, console_recv) = mpsc::channel();

        let args = env::args()
            .zip(env::args().skip(1))
            .filter(|(cmd, _)| VALID_RCON_ARGS.contains(&&cmd[..]))
            .fold(HashMap::new(), |mut hash_map, (cmd, arg)| {
                _ = hash_map.insert(cmd, arg);
                hash_map
            });

        std::thread::spawn(move || _ = run_rcon(args));

        Self {
            rcon_tasks: Mutex::new(recv),
            rcon_send_tasks: Mutex::new(sender),
            console_sender: Mutex::new(console_sender),
            console_recv: Mutex::new(console_recv),
        }
    }

    fn on_dll_load(&self, engine: Option<&EngineData>, dll_ptr: &DLLPointer) {
        unsafe { EngineFunctions::try_init(dll_ptr, &ENGINE_FUNCTIONS) };

        if let WhichDll::Client = dll_ptr.which_dll() {
            let addr = dll_ptr.get_dll_ptr() as isize;
            std::thread::spawn(move || _ = hook_console_print(addr));
        }

        let engine = if let Some(engine) = engine {
            engine
        } else {
            return;
        };

        engine
            .register_concommand("test_cnet", test_cnet, "", 0)
            .unwrap();
    }

    fn runframe(&self) {
        // can be moved somewhere else

        let funcs = ENGINE_FUNCTIONS.wait();

        if let Ok(task) = self.rcon_tasks.lock().try_recv() {
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

fn run_rcon(rcon_args: HashMap<String, String>) -> Option<std::convert::Infallible> {
    let mut server = match RconServer::try_new(
        rcon_args.get(VALID_RCON_ARGS[0])?,
        rcon_args.get(VALID_RCON_ARGS[1])?.to_string(),
    ) {
        Ok(sv) => sv,
        Err(err) => {
            log::info!("failed to connect to socket : {err:?}");
            return None;
        }
    };

    hook_write_console();

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

#[rrplug::concommand]
fn test_cnet() -> Option<()> {
    unsafe {
        let client: *const CClient = ENGINE_FUNCTIONS.wait().client_array.add(0).as_ref()?;

        let cnet_channel = client.offset(0x290) as *const c_void;

        dbg!(cnet_channel);
    }

    None
}
