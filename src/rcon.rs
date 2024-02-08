use rrplug::mid::utils::{to_cstring, try_cstring};
use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc::Receiver,
};
use thiserror::Error;

use crate::{
    bindings::{CmdSource, ENGINE_FUNCTIONS},
    console::ConsoleAccess,
};

const SERVERDATA_AUTH: i32 = 3;
const SERVERDATA_EXECCOMMAND: i32 = 2;
const SERVERDATA_AUTH_RESPONSE: i32 = 2;
const SERVERDATA_RESPONSE_VALUE: i32 = 0;
const MAX_PACKET_SIZE: usize = 4096;
const MIN_PACKET_SIZE: usize = 10;
pub const MAX_CONTENT_SIZE: usize = MAX_PACKET_SIZE - MIN_PACKET_SIZE;

#[derive(Debug, Error)]
pub enum RconRequestError {
    #[error("invalid vec length; converstion failed")]
    VecToArrayError,

    #[error("invalid request type {0}")]
    InvalidRequestType(i32),

    #[error("a cast failed")]
    IntCastFail,

    #[error("read the wrong amount of bytes out of a stream {MAX_CONTENT_SIZE} got {0}")]
    ReadWrongAmount(usize),

    #[error("the connect client provided a invalid id to run a command which was {0}")]
    InvalidClientID(i32),

    #[error(transparent)]
    SocketError(#[from] std::io::Error),
}

pub struct RconResponse {
    id: i32,
    ty: i32,
    content: String,
}

#[allow(clippy::from_over_into)]
impl Into<Vec<u8>> for RconResponse {
    fn into(self) -> Vec<u8> {
        let mut buf = Vec::new();

        self.id.to_le_bytes().into_iter().for_each(|b| buf.push(b));
        self.ty.to_le_bytes().into_iter().for_each(|b| buf.push(b));
        self.content
            .into_bytes()
            .into_iter()
            .for_each(|b| buf.push(b));
        buf.push(b'\0');
        buf.push(b'\0');

        // size
        (buf.len() as i32)
            .to_le_bytes()
            .into_iter()
            .rev()
            .for_each(|b| buf.insert(0, b));

        buf
    }
}

pub struct RconStream {
    pub stream: TcpStream,
    pub auth: bool,
}

pub struct RconServer {
    password: String,
    server: TcpListener,
    connections: Vec<RconStream>,
    console: ConsoleAccess,
}

impl RconServer {
    pub fn try_new(
        bind_ip: &str,
        password: impl Into<String>,
        console_recv: Receiver<String>,
    ) -> Result<Self, std::io::Error> {
        let server = TcpListener::bind(bind_ip)?;

        server.set_nonblocking(true)?;

        let rcon_server = Self {
            password: password.into(),
            server,
            connections: Vec::new(),
            console: ConsoleAccess::new(console_recv),
        };

        Ok(rcon_server)
    }

    pub fn run(&mut self) {
        while self.console.next_line_catpure().is_some() {} // string allocation could be remove

        match self.server.accept() {
            Ok((conn, addr)) => match conn.set_nonblocking(true) {
                Ok(_) => {
                    log::info!("connection created with {addr:?}");
                    self.connections.push(RconStream {
                        stream: conn,
                        auth: false,
                    })
                }
                Err(err) => log::error!("failed to connect to a stream: {err}"),
            },
            Err(err) => match err.kind() {
                io::ErrorKind::WouldBlock => {}
                _ => log::warn!("connection failed because of {err}"),
            },
        }

        for i in 0..self.connections.len() {
            match handle_connection(&mut self.connections[i], &self.password, &mut self.console) {
                Ok(_) => {}
                Err(err) => {
                    match &err {
                        RconRequestError::SocketError(err) => match err.kind() {
                            io::ErrorKind::WouldBlock => continue,
                            _ => log::error!("{err}"),
                        },
                        _ => log::error!("{err}"),
                    }

                    log::info!("terminating a connection");
                    self.connections.remove(i); // this doesn't seam to prevent future connections from the same connection O_o
                    break;
                }
            }
        }
    }
}

pub fn handle_connection(
    conn: &mut RconStream,
    password: &str,
    console: &mut ConsoleAccess,
) -> Result<(), RconRequestError> {
    let (client_id, request_type, content) = read_rcon_stream(&mut conn.stream)?;

    let response = parse_response(conn, password, console, client_id, request_type, content)?;

    let buf: Vec<u8> = response.into();
    conn.stream.write_all(&buf)?;

    Ok(())
}

fn read_rcon_stream(stream: &mut TcpStream) -> Result<(i32, i32, String), RconRequestError> {
    let mut size_buf = vec![0; 4];
    let bytes_read = stream.read(&mut size_buf)?;
    if bytes_read != 4 {
        Err(RconRequestError::ReadWrongAmount(bytes_read))?
    }

    let size = i32::from_le_bytes(
        size_buf
            .into_iter()
            .collect::<Vec<u8>>()
            .try_into()
            .or(Err(RconRequestError::VecToArrayError))?,
    )
    .try_into()
    .or(Err(RconRequestError::IntCastFail))?;

    let mut buf = vec![0; size];

    let bytes_read = stream.read(&mut buf)?;
    if bytes_read != size && size <= MAX_PACKET_SIZE {
        Err(RconRequestError::ReadWrongAmount(bytes_read))?
    }

    let client_id = i32::from_le_bytes(
        buf.drain(..=3)
            .collect::<Vec<u8>>()
            .try_into()
            .or(Err(RconRequestError::VecToArrayError))?,
    );

    let request_type = i32::from_le_bytes(
        buf.drain(..=3)
            .collect::<Vec<u8>>()
            .try_into()
            .or(Err(RconRequestError::VecToArrayError))?,
    );

    Ok((
        client_id,
        request_type,
        String::from_utf8_lossy(&buf).to_string().replace('\0', ""),
    )) // TODO: maybe use a struct
}

fn parse_response(
    conn: &mut RconStream,
    password: &str,
    console: &mut ConsoleAccess,
    client_id: i32,
    request_type: i32,
    content: String,
) -> Result<RconResponse, RconRequestError> {
    let response = match request_type {
        SERVERDATA_AUTH => {
            if content == password {
                conn.auth = true;

                log::info!("auth successful");

                RconResponse {
                    id: client_id,
                    ty: SERVERDATA_AUTH_RESPONSE,
                    content: String::new(),
                }
            } else {
                log::warn!("auth failed");
                conn.auth = false;

                RconResponse {
                    id: -1,
                    ty: SERVERDATA_AUTH_RESPONSE,
                    content: String::new(),
                }
            }
        }
        SERVERDATA_EXECCOMMAND if content == "dumpconsole" => {
            if !conn.auth {
                Err(RconRequestError::InvalidClientID(client_id))?
            }
            log::info!("sending console dump");

            RconResponse {
                id: client_id,
                ty: SERVERDATA_RESPONSE_VALUE,
                content: console.get_last_console_output().iter().cloned().collect(),
            }
        }
        SERVERDATA_EXECCOMMAND => {
            if !conn.auth {
                Err(RconRequestError::InvalidClientID(client_id))?
            }
            log::info!("executing command : {content}");

            let cmd = try_cstring(&content)
                .unwrap_or_else(|_| to_cstring(content.replace('\0', "").as_str()));
            let funcs = ENGINE_FUNCTIONS.wait();
            unsafe {
                (funcs.cbuf_add_text_type)(
                    (funcs.cbuf_get_current_player)(),
                    cmd.as_ptr(),
                    CmdSource::Code,
                );

                (funcs.cbuf_execute)() // execute the buffer rn since we want the results immediately
            }

            let mut response = String::new();
            while let Some(console_out) = console.next_line_catpure() {
                response += &console_out;
            }

            RconResponse {
                id: client_id,
                ty: SERVERDATA_RESPONSE_VALUE,
                content: response,
            }
        }
        request_num => Err(RconRequestError::InvalidRequestType(request_num))?,
    };

    Ok(response)
}
