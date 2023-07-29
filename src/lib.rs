use bindings::{CmdSource, EngineFunctions, ENGINE_FUNCTIONS};
use parking_lot::Mutex;
use rcon::{RconServer, RconTask};
use rrplug::{prelude::*, to_sq_string, bindings::entity::CBaseClient};
use std::{
    collections::HashMap,
    env,
    sync::mpsc::{self, Receiver, Sender}, ffi::c_void,
};

pub mod bindings;
pub mod rcon;

const VALID_RCON_ARGS: [&str; 2] = ["rcon_ip_port", "rcon_password"];

#[derive(Debug)]
pub struct RconPlugin {
    rcon_tasks: Mutex<Receiver<RconTask>>,
    pub rcon_send_tasks: Mutex<Sender<RconTask>>, // mutex is not needed but it must sync so clone on each thread
    rcon_args: HashMap<String, String>,
}

impl Plugin for RconPlugin {
    fn new(_: &PluginData) -> Self {
        let (sender, recv) = mpsc::channel();

        let args = env::args()
            .zip(env::args().skip(1))
            .filter(|(cmd, _)| VALID_RCON_ARGS.contains(&&cmd[..]))
            .fold(HashMap::new(), |mut hash_map, (cmd, arg)| {
                _ = hash_map.insert(cmd, arg);
                hash_map
            });

        // if args.len() != VALID_RCON_ARGS.len() {
        //     panic!("not all rcon cmd args entered")
        // }

        Self {
            rcon_tasks: Mutex::new(recv),
            rcon_send_tasks: Mutex::new(sender),
            rcon_args: args,
        }
    }

    fn main(&self) {
        let mut server = match RconServer::try_new(
            self.rcon_args
                .get(VALID_RCON_ARGS[0])
                .expect("a rcon cmd wasn't present"),
            self.rcon_args
                .get(VALID_RCON_ARGS[1])
                .expect("a rcon cmd wasn't present")
                .to_string(),
        ) {
            Ok(sv) => sv,
            Err(err) => return log::info!("failed to connect to socket : {err:?}"),
        };
        let rcon_send_tasks = self.rcon_send_tasks.lock();

        loop {
            if let Some(tasks) = server.run() {
                tasks
                    .into_iter()
                    .for_each(|task| rcon_send_tasks.send(task).expect("failed to send tasks"))
            }
        }
    }

    fn on_engine_load(&self, engine: &EngineLoadType, dll_ptr: DLLPointer) {
        unsafe { EngineFunctions::try_init(&dll_ptr, &ENGINE_FUNCTIONS) };

        let engine = if let EngineLoadType::Engine(engine) = *engine {
            engine
        } else {
            return;
        };

        engine.register_concommand("test_cnet", test_cnet, "", 0).unwrap();
    }

    fn runframe(&self) {
        // can be moved somewhere else

        let funcs = ENGINE_FUNCTIONS.wait();

        if let Ok(task) = self.rcon_tasks.lock().try_recv() {
            match task {
                RconTask::Runcommand(cmd) => unsafe {
                    log::info!("executing command : {cmd}");

                    let cmd = to_sq_string!(cmd);
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

entry!(RconPlugin);

#[rrplug::concommand]
fn test_cnet() -> Option<()> {
    unsafe {
        let client: *const CBaseClient = ENGINE_FUNCTIONS.wait().client_array.add(0).as_ref()?;

        let cnet_channel = client.offset(0x290) as *const c_void;

        dbg!(cnet_channel);
    }

    None
}