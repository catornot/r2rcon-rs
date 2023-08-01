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

#[derive(Debug, Error)]
pub enum RconRequestError {
    #[error("invalid vec length; converstion failed")]
    VecToArrayError,

    #[error("invalid request type")]
    InvalidRequestType,

    #[error("a cast failed")]
    IntCastFail,

    #[error("read the wrong amount of bytes out of a stream")]
    ReadWrongAmount,

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
    pub id: i32,
}

pub struct RconServer {
    password: String,
    server: TcpListener,
    connections: Vec<RconStream>,
    tasks: Vec<RconTask>,
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
        };

        Ok(rcon_server)
    }

    pub fn run(&mut self) -> Option<Vec<RconTask>> {
        match self.server.accept() {
            Ok((conn, addr)) => match conn.set_nonblocking(true) {
                Ok(_) => {
                    log::info!("connection created with {addr:?}");
                    self.connections.push(RconStream {
                        stream: conn,
                        id: -1,
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
            match handle_connection(&mut self.connections[i], &self.password) {
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
                    self.connections.remove(i);
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
) -> Result<Option<RconTask>, RconRequestError> {
    let stream = &mut conn.stream;

    let mut size_buf = vec![0; 4];
    if stream.read(&mut size_buf)? != 4 {
        Err(RconRequestError::ReadWrongAmount)?
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
    if stream.read(&mut buf)? != size && size <= MAX_PACKET_SIZE {
        Err(RconRequestError::ReadWrongAmount)?
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
                conn.id = client_id;

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
            if conn.id == -1 {
                Err(RconRequestError::InvalidClientID(client_id))?
            }

            (
                RconResponse {
                    id: conn.id,
                    ty: SERVERDATA_RESPONSE_VALUE,
                    content: String::from("uh idk idk how to read console send help"),
                },
                Some(RconTask::Runcommand(content)),
            )
        }
        _ => Err(RconRequestError::InvalidRequestType)?,
    };

    let buf: Vec<u8> = response.into();
    stream.write_all(&buf)?;

    Ok(task)
}
