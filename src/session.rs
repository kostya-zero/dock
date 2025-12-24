use std::{net::SocketAddr, path::PathBuf};

use anyhow::Result;
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::{commands::Commands, config::Config};

macro_rules! reply {
    ($self:expr, $code:expr, $message:expr) => {
        $self.reply($code, $message).await?;
    };
}

macro_rules! reply_ok {
    ($self:expr, $code:expr, $message:expr) => {
        $self.reply($code, $message).await?;
        return Ok(());
    };
}

#[derive(Debug, Error)]
pub enum ConnectionError {
    #[error("user has disconnected")]
    Disconnected,

    #[error("read error: {0}")]
    ReadFailed(String),

    #[error("write error: {0}")]
    WriteError(String),
}

#[derive(Debug)]
#[allow(unused)]
pub struct Session {
    username: String,
    authorized: bool,
    current_dir: PathBuf,
    connection: TcpStream,
    address: SocketAddr,
    config: Config,
    id: String,
}

impl Session {
    pub fn new(id: &String, address: SocketAddr, connection: TcpStream, config: Config) -> Self {
        Self {
            id: id.to_owned(),
            address,
            connection,
            config,
            current_dir: PathBuf::from("/"),
            username: String::new(),
            authorized: false,
        }
    }
    async fn receive(&mut self) -> Result<String, ConnectionError> {
        let mut buf = [0u8; 1024];
        let n = match self.connection.read(&mut buf).await {
            Ok(0) => return Err(ConnectionError::Disconnected),
            Ok(n) => n,
            Err(e) => return Err(ConnectionError::ReadFailed(e.to_string())),
        };
        let data = String::from_utf8_lossy(&buf[..n]);
        Ok(data.to_string())
    }

    async fn split_data(&self, data: String) -> Option<(String, String)> {
        let splitted = data
            .splitn(2, ' ')
            .map(String::from)
            .collect::<Vec<String>>();
        if splitted.is_empty() || splitted.len() != 2 {
            return None;
        }

        let command = splitted.first().unwrap().to_owned();
        let arg = splitted.last().unwrap().to_owned();
        Some((command, arg))
    }

    async fn reply(&mut self, code: u16, message: &str) -> Result<(), ConnectionError> {
        let formatted_message = format!("{code} {message}");
        if let Err(e) = self
            .connection
            .write_all(formatted_message.as_bytes())
            .await
        {
            return Err(ConnectionError::WriteError(e.to_string()));
        }
        Ok(())
    }

    #[allow(unused)]
    #[must_use = "there could be a connection related error"]
    pub async fn run_session(&mut self) -> Result<()> {
        self.reply(220, "Dock is welcoming you!").await?;
        loop {
            let data = self.receive().await?;
            let (cmd, arg) = if let Some((c, a)) = self.split_data(data).await {
                (c, a)
            } else {
                eprintln!("bad format.");
                continue;
            };

            let command: Commands = cmd.into();
            self.handle_command(command, arg).await?;
        }
        Ok(())
    }

    async fn handle_command(&mut self, cmd: Commands, arg: String) -> Result<()> {
        match cmd {
            Commands::User => {
                if arg.is_empty() {
                    reply_ok!(self, 510, "Username is required.");
                }

                if !self.config.check_user(&arg) {
                    reply_ok!(self, 530, "Authorization failed.");
                }

                self.username = arg;
                reply!(self, 331, "Password is required");
            }
            Commands::Password => {
                if self.username.is_empty() {
                    reply_ok!(self, 510, "Username is required.");
                }

                if arg.is_empty() {
                    reply_ok!(self, 510, "Password is required");
                }

                if !self.config.check_password(&self.username, &arg) {
                    reply_ok!(self, 530, "Authorization failed.");
                }

                self.authorized = true;
                reply!(self, 230, "Login success.");
            }
            Commands::Unknown => todo!(),
        }
        Ok(())
    }

    pub fn id(&self) -> &String {
        &self.id
    }
}
