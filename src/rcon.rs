use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
};
use thiserror::Error;

#[derive(Debug, Clone)]
pub enum RconTask {
    Runcommand(String),
}

// this could be fun https://discord.com/channels/920776187884732556/922663696273125387/1134900622773194782

const SERVERDATA_AUTH: i32 = 3;
const SERVERDATA_EXECCOMMAND: i32 = 2;
const SERVERDATA_AUTH_RESPONSE: i32 = 2;
const SERVERDATA_RESPONSE_VALUE: i32 = 0;
const MAX_PACKET_SIZE: usize = 4096;
const MIN_PACKET_SIZE: usize = 10;
const MAX_CONTENT_SIZE: usize = MAX_PACKET_SIZE - MIN_PACKET_SIZE;

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
    tasks: Vec<RconTask>,
    cmd_buffer: Vec<String>,
}

impl RconServer {
    pub fn try_new(bind_ip: &str, password: String) -> Result<Self, std::io::Error> {
        let server = TcpListener::bind(bind_ip)?;

        server.set_nonblocking(true)?;

        let rcon_server = Self {
            password,
            server,
            connections: Vec::new(),
            tasks: Vec::new(),
            cmd_buffer: Vec::new(),
        };

        Ok(rcon_server)
    }

    pub fn run(&mut self, new_console_line: Option<String>) -> Option<Vec<RconTask>> {
        if let Some(line) = new_console_line {
            let line_size = line.len();
            let mut buffer_size = 0;

            for bline in self.cmd_buffer.drain(..).rev().collect::<Vec<String>>() {
                buffer_size += bline.len();
                if buffer_size + line_size > MAX_CONTENT_SIZE {
                    break;
                }

                self.cmd_buffer.insert(0, bline);
            }
            self.cmd_buffer.push(line);
        }

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
            match handle_connection(&mut self.connections[i], &self.password, &self.cmd_buffer) {
                Ok(maybe_task) => {
                    if let Some(task) = maybe_task {
                        self.tasks.push(task)
                    }
                }
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

        if self.tasks.is_empty() {
            None
        } else {
            Some(self.tasks.drain(0..).collect())
        }
    }
}

pub fn handle_connection(
    conn: &mut RconStream,
    password: &str,
    cmd_buffer: &[String],
) -> Result<Option<RconTask>, RconRequestError> {
    let stream = &mut conn.stream;

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

    let content = String::from_utf8_lossy(&buf).to_string().replace('\0', "");

    let (response, task) = match request_type {
        SERVERDATA_AUTH => {
            if content == password {
                conn.auth = true;

                log::info!("auth successful");

                (
                    RconResponse {
                        id: client_id,
                        ty: SERVERDATA_AUTH_RESPONSE,
                        content: String::new(),
                    },
                    None,
                )
            } else {
                log::warn!("auth failed");
                conn.auth = false;

                (
                    RconResponse {
                        id: -1,
                        ty: SERVERDATA_AUTH_RESPONSE,
                        content: String::new(),
                    },
                    None,
                )
            }
        }
        SERVERDATA_EXECCOMMAND => {
            if !conn.auth {
                Err(RconRequestError::InvalidClientID(client_id))?
            }

            (
                RconResponse {
                    id: client_id,
                    ty: SERVERDATA_RESPONSE_VALUE,
                    content: cmd_buffer.iter().cloned().collect(),
                },
                Some(RconTask::Runcommand(content)),
            )
        }
        request_num => Err(RconRequestError::InvalidRequestType(request_num))?,
    };

    let buf: Vec<u8> = response.into();
    stream.write_all(&buf)?;

    Ok(task)
}
