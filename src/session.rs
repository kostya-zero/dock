use std::{
    fs::Permissions,
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    time::Duration,
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use anyhow::{Result, anyhow, bail};
use thiserror::Error;
use tokio::{
    fs::{self, File},
    io::{self, AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom},
    net::{TcpListener, TcpStream},
    time,
};
use tracing::info;

use crate::{commands::Commands, config::Config};

const SERVER_FEATURES: [&str; 4] = ["UTF8", "MLST type*;size*;modify*;perm*;", "PASV", "PORT"];
const DISALLOWED_FILENAMES: [&str; 2] = ["..", "."];

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

macro_rules! require_authorization {
    ($self:expr) => {
        if !$self.authorized {
            $self.reply(530, "Login is required.").await?;
            return Ok(());
        }
    };
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConnectionError {
    #[error("user has disconnected")]
    Disconnected,

    #[error("read error: {0}")]
    ReadFailed(String),

    #[error("write error: {0}")]
    WriteError(String),

    #[error("session ended manually by client")]
    ClosedByQuit,

    #[error("data connection failed: {0}")]
    DataConnectionFailed(String),

    #[error("file system error occurred")]
    FileSystemError,
}

#[derive(Debug)]
pub struct Session {
    username: String,
    authorized: bool,
    current_dir: PathBuf,
    connection: TcpStream,
    rest_offset: u64,
    active_addr: Option<SocketAddr>,
    passive_listener: Option<TcpListener>,
    config: Config,
    id: String,
}

impl Session {
    pub fn new(id: &String, connection: TcpStream, config: Config) -> Self {
        Self {
            id: id.to_owned(),
            connection,
            config,
            rest_offset: 0,
            active_addr: None,
            passive_listener: None,
            current_dir: PathBuf::from("/"),
            username: String::new(),
            authorized: false,
        }
    }

    /// Formats file permissions in Unix format (e.g., drwxr-xr-x)
    fn format_unix_permissions(is_dir: bool, permissions: &Permissions) -> String {
        let mut perms = String::with_capacity(10);

        // File type
        perms.push(if is_dir { 'd' } else { '-' });

        // Get Unix permissions or use defaults for non-Unix systems
        #[cfg(unix)]
        let mode = permissions.mode();

        #[cfg(not(unix))]
        let mode = if permissions.readonly() {
            0o444 // r--r--r--
        } else {
            0o644 // rw-r--r--
        };

        // Owner permissions
        perms.push(if mode & 0o400 != 0 { 'r' } else { '-' });
        perms.push(if mode & 0o200 != 0 { 'w' } else { '-' });
        perms.push(if mode & 0o100 != 0 { 'x' } else { '-' });

        // Group permissions
        perms.push(if mode & 0o040 != 0 { 'r' } else { '-' });
        perms.push(if mode & 0o020 != 0 { 'w' } else { '-' });
        perms.push(if mode & 0o010 != 0 { 'x' } else { '-' });

        // Others permissions
        perms.push(if mode & 0o004 != 0 { 'r' } else { '-' });
        perms.push(if mode & 0o002 != 0 { 'w' } else { '-' });
        perms.push(if mode & 0o001 != 0 { 'x' } else { '-' });

        perms
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
        let trimmed = data.trim_end();
        let splitted = trimmed
            .splitn(2, ' ')
            .map(String::from)
            .collect::<Vec<String>>();
        if splitted.is_empty() {
            return None;
        }

        let command = splitted.first().unwrap().to_owned();
        let arg = if splitted.len() > 1 {
            splitted.last().unwrap().to_owned()
        } else {
            String::new()
        };
        Some((command, arg))
    }

    async fn reply(&mut self, code: u16, message: &str) -> Result<(), ConnectionError> {
        let formatted_message = format!("{code} {message}\r\n");
        if let Err(e) = self
            .connection
            .write_all(formatted_message.as_bytes())
            .await
        {
            return Err(ConnectionError::WriteError(e.to_string()));
        }
        Ok(())
    }
    async fn reply_without_code(&mut self, message: &str) -> Result<(), ConnectionError> {
        let formatted_message = format!("{message}\r\n");
        if let Err(e) = self
            .connection
            .write_all(formatted_message.as_bytes())
            .await
        {
            return Err(ConnectionError::WriteError(e.to_string()));
        }
        Ok(())
    }

    #[must_use = "there could be a connection related error"]
    pub async fn run_session(&mut self) -> Result<(), ConnectionError> {
        self.reply(220, "Dock is welcoming you!").await?;
        loop {
            let data = self.receive().await?;
            let (cmd, arg) = if let Some((c, a)) = self.split_data(data).await {
                (c, a)
            } else {
                continue;
            };

            let command: Commands = cmd.into();
            self.handle_command(command, arg).await?;
        }
    }

    async fn handle_command(&mut self, cmd: Commands, arg: String) -> Result<(), ConnectionError> {
        match cmd {
            Commands::User => {
                if self.authorized {
                    reply_ok!(self, 230, "Already logged in.");
                }

                if arg.is_empty() {
                    reply_ok!(self, 501, "Username is required.");
                }

                if !self.config.check_user(&arg) {
                    reply_ok!(self, 530, "Authorization failed.");
                }

                self.username = arg;
                reply!(self, 331, "Password is required");
            }
            Commands::Password => {
                if self.username.is_empty() {
                    reply_ok!(self, 501, "Username is required.");
                }

                if arg.is_empty() {
                    reply_ok!(self, 501, "Password is required");
                }

                if !self.config.check_password(&self.username, &arg) {
                    reply_ok!(self, 530, "Authorization failed.");
                }

                self.authorized = true;
                info!(session_id=%self.id, username=%self.username, "User authorized.");
                reply!(self, 230, "Login success.");
            }
            Commands::WorkingDir => {
                reply!(
                    self,
                    257,
                    format!(
                        "\"{}\" is the current directory.",
                        self.current_dir.to_string_lossy()
                    )
                    .as_str()
                );
            }
            Commands::ChangeDir => {
                require_authorization!(self);

                if arg.is_empty() {
                    reply_ok!(self, 501, "Path is required");
                }

                let temp_cwd = self.current_dir.join(arg);
                let temp_cwd_string = temp_cwd.to_string_lossy().to_string();
                let trimmed_temp_cwd = temp_cwd_string.trim_start_matches("/");
                let real_path = PathBuf::from(&self.config.root).join(trimmed_temp_cwd);
                if !real_path.exists() {
                    reply_ok!(self, 550, "Path does not exist.");
                }
                if !real_path.is_dir() {
                    reply_ok!(self, 550, "Not a directory.");
                }

                self.current_dir = temp_cwd;
                reply!(self, 250, "Directory changed.");
            }
            Commands::Option => {
                if arg.is_empty() {
                    reply_ok!(self, 501, "Argument is required");
                }

                match arg.as_str() {
                    "UTF8" => {
                        reply!(self, 200, "UTF-8 is enabled by default.");
                    }
                    _ => {
                        reply!(self, 501, "Unknown option");
                    }
                }
            }
            Commands::List => {
                require_authorization!(self);
                let mut data_connection = self
                    .open_data_connection()
                    .await
                    .map_err(|e| ConnectionError::DataConnectionFailed(e.to_string()))?;
                reply!(self, 150, "Listing of directory");

                let cwd = self.current_dir.to_string_lossy().to_string();
                let trimmed_cwd = cwd.trim_start_matches("/");
                let real_path = Path::new(&self.config.root).join(trimmed_cwd);

                // Pseudo values. I dont think clients really care about it.
                let links = "1";
                let owner = "root";
                let group = "group";

                let mut entries = fs::read_dir(real_path)
                    .await
                    .map_err(|_| ConnectionError::FileSystemError)?;

                let mut listing_strings: Vec<String> = Vec::new();

                while let Some(entry) = entries
                    .next_entry()
                    .await
                    .map_err(|_| ConnectionError::FileSystemError)?
                {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let metadata = entry
                        .metadata()
                        .await
                        .map_err(|_| ConnectionError::FileSystemError)?;

                    let is_dir = metadata.is_dir();
                    let size = metadata.len();
                    let perms = Self::format_unix_permissions(is_dir, &metadata.permissions());

                    // Format: permissions links owner group size month day time name
                    // Example: drwxr-xr-x 1 root group 4096 Jan 01 12:00 dirname
                    let modified = metadata
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);

                    // Simple timestamp formatting (could be improved with chrono)
                    let timestamp = format_timestamp(modified);

                    let line = format!(
                        "{} {} {} {} {:>12} {} {}\r\n",
                        perms, links, owner, group, size, timestamp, name
                    );
                    listing_strings.push(line);
                }

                // Send listing through data connection
                for entry in listing_strings {
                    data_connection
                        .write_all(entry.as_bytes())
                        .await
                        .map_err(|e| ConnectionError::WriteError(e.to_string()))?;
                }

                let _ = data_connection.shutdown().await;
                reply!(self, 226, "Transfer complete.");
            }
            Commands::Quit => {
                reply!(self, 221, "Bye!");
                return Err(ConnectionError::ClosedByQuit);
            }
            Commands::Features => {
                reply!(self, 211, "Features");
                for i in SERVER_FEATURES {
                    self.reply_without_code(i).await?;
                }
                reply!(self, 211, "End");
            }
            Commands::Unknown => {
                reply!(self, 502, "Unknown command.");
            }
            Commands::System => {
                reply!(self, 215, "UNIX Type: L8");
            }
            Commands::Type => {
                reply!(self, 200, "OK");
            }
            Commands::Size => {
                require_authorization!(self);

                if arg.is_empty() {
                    reply_ok!(self, 501, "Path is required");
                }

                let temp_path = self.current_dir.join(arg);
                let temp_path_string = temp_path.to_string_lossy().to_string();
                let trimmed_temp_path = temp_path_string.trim_start_matches("/");
                let real_path = PathBuf::from(&self.config.root).join(trimmed_temp_path);
                if !real_path.exists() {
                    reply_ok!(self, 550, "Path does not exist.");
                }
                if !real_path.is_file() {
                    reply_ok!(self, 550, "Not a file.");
                }

                let metadata = fs::metadata(real_path)
                    .await
                    .map_err(|_| ConnectionError::FileSystemError)?;
                let size = metadata.len();

                reply!(self, 213, format!("{}", size).as_str());
            }
            Commands::ChangeDirectoryUp => {
                require_authorization!(self);

                let parent = if let Some(p) = self.current_dir.parent() {
                    p.to_path_buf()
                } else {
                    PathBuf::from("/")
                };
                self.current_dir = parent;
                reply!(self, 250, "Directory changed.");
            }
            Commands::Port => {
                require_authorization!(self);

                if arg.is_empty() {
                    reply_ok!(self, 501, "Address is required");
                }

                let splitted: Vec<String> = arg.split(',').map(String::from).collect();
                if splitted.len() != 6 {
                    reply_ok!(self, 501, "Syntax error in arguments");
                }

                let h1 = splitted[0].trim();
                let h2 = splitted[1].trim();
                let h3 = splitted[2].trim();
                let h4 = splitted[3].trim();

                let p1: usize = splitted[4].trim().parse().unwrap();
                if p1 > 255 {
                    reply_ok!(self, 501, "Invalid port.");
                }

                let p2: usize = splitted[5].trim().parse().unwrap();
                if p2 > 255 {
                    reply_ok!(self, 501, "Invalid port.");
                }

                let port = p1 * 256 + p2;
                let ip_string = format!("{h1}.{h2}.{h3}.{h4}:{port}");
                let addr: SocketAddr = ip_string.parse().unwrap();

                if let Some(pasv) = self.passive_listener.take() {
                    drop(pasv);
                    self.passive_listener = None;
                }

                self.active_addr = Some(addr);
                reply!(self, 200, "PORT command success.");
            }
            Commands::Passive => {
                require_authorization!(self);
                let ln = TcpListener::bind("0.0.0.0:0")
                    .await
                    .map_err(|_| ConnectionError::FileSystemError)?;
                let addr: SocketAddr = ln
                    .local_addr()
                    .map_err(|_| ConnectionError::FileSystemError)?;
                let port = addr.port();

                self.passive_listener = Some(ln);

                let ip = match self
                    .connection
                    .local_addr()
                    .map_err(|_| ConnectionError::FileSystemError)?
                {
                    SocketAddr::V4(v4) if !v4.ip().is_unspecified() => *v4.ip(),
                    _ => Ipv4Addr::new(127, 0, 0, 1),
                };

                let [h1, h2, h3, h4] = ip.octets();
                let p1 = port / 256;
                let p2 = port % 256;

                reply!(
                    self,
                    227,
                    format!(
                        "Entering Passive Mode ({},{},{},{},{},{})",
                        h1, h2, h3, h4, p1, p2
                    )
                    .as_str()
                );
            }
            Commands::Rest => {
                require_authorization!(self);

                if arg.is_empty() {
                    reply_ok!(self, 501, "Argument is required.");
                }

                self.rest_offset = arg.parse().unwrap();
                reply!(self, 350, "Restarting at sepcific bytes.");
            }
            Commands::Retrive => {
                require_authorization!(self);

                if !self.config.can_user_read(&self.username) {
                    reply_ok!(self, 501, "No permission to read.");
                }

                if arg.is_empty() {
                    reply_ok!(self, 501, "Argument is required.");
                }

                let real_path = self.get_real_path().await;
                let file_path = real_path.join(arg);
                if !file_path.exists() {
                    reply_ok!(self, 550, "File not found.");
                }
                let mut file = File::open(&file_path)
                    .await
                    .map_err(|_| ConnectionError::FileSystemError)?;
                let meta = file
                    .metadata()
                    .await
                    .map_err(|_| ConnectionError::FileSystemError)?;
                let size = meta.len();

                if self.rest_offset > 0 {
                    if self.rest_offset >= size {
                        self.rest_offset = 0;
                        reply_ok!(self, 550, "Invalid restart position.");
                    }
                    file.seek(SeekFrom::Start(self.rest_offset))
                        .await
                        .map_err(|_| ConnectionError::FileSystemError)?;
                }

                if let Ok(mut data) = self.open_data_connection().await {
                    reply!(self, 150, "Ready to transfer...");
                    info!(session_id=%self.id, file=%file_path.to_string_lossy() , username=%self.username, "User is retriving file.");
                    io::copy(&mut file, &mut data).await.map_err(|_| {
                        ConnectionError::DataConnectionFailed(String::from("I/O operation failed"))
                    })?;
                    let _ = data.shutdown().await;
                    reply!(self, 226, "Done.");
                } else {
                    reply!(self, 425, "Cant open data connection.");
                }
            }
            Commands::Store => {
                require_authorization!(self);

                if !self.config.can_user_write(&self.username) {
                    reply_ok!(self, 550, "No permission to write.");
                }

                if arg.is_empty() {
                    reply_ok!(self, 501, "Argument is required.");
                }

                if DISALLOWED_FILENAMES.contains(&arg.as_str()) {
                    reply_ok!(self, 553, "File name not allowed.");
                }

                let real_path = self.get_real_path().await;
                let file_path = real_path.join(arg);
                let parent_dir = file_path.parent().unwrap_or(Path::new(""));
                fs::create_dir_all(parent_dir)
                    .await
                    .map_err(|_| ConnectionError::FileSystemError)?;
                let mut file = File::create(&file_path)
                    .await
                    .map_err(|_| ConnectionError::FileSystemError)?;

                let mut data = self
                    .open_data_connection()
                    .await
                    .map_err(|e| ConnectionError::DataConnectionFailed(e.to_string()))?;

                io::copy(&mut data, &mut file).await.map_err(|_| {
                    ConnectionError::DataConnectionFailed(String::from("I/O operation failed"))
                })?;

                let _ = data.shutdown().await;
                reply!(self, 226, "Transfer complete.");
            }
        }
        Ok(())
    }

    async fn open_data_connection(&mut self) -> Result<TcpStream, anyhow::Error> {
        let timeout = Duration::from_secs(10);

        // Active Mode (PORT)
        if let Some(addr) = self.active_addr.take() {
            let stream = time::timeout(timeout, TcpStream::connect(&addr))
                .await
                .map_err(|_| anyhow!("data connection timeout"))?
                .map_err(anyhow::Error::from)?;
            return Ok(stream);
        }

        // Passive Mode (PASV)
        let listener = match self.passive_listener.take() {
            Some(l) => l,
            None => bail!("use PASV or PORT first"),
        };

        let accept_fn = async move {
            let (stream, _) = listener.accept().await?;
            Ok::<TcpStream, anyhow::Error>(stream)
        };

        let stream = time::timeout(timeout, accept_fn)
            .await
            .map_err(|_| anyhow!("data connection timeout"))??;

        Ok(stream)
    }

    pub fn id(&self) -> &String {
        &self.id
    }

    async fn get_real_path(&mut self) -> PathBuf {
        let temp_cwd = &self.current_dir;
        let temp_cwd_string = temp_cwd.to_string_lossy().to_string();
        let temp_cwd_trimmed = temp_cwd_string.trim_start_matches('/');
        Path::new(&self.config.root).join(temp_cwd_trimmed)
    }
}

/// Formats a Unix timestamp into a simple date-time string
/// Format: "Mon DD HH:MM" or "Mon DD  YYYY" for older files
fn format_timestamp(timestamp: u64) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let six_months = 60 * 60 * 24 * 180;
    let time = UNIX_EPOCH + std::time::Duration::from_secs(timestamp);

    // Simple formatting - in production you'd use chrono
    let datetime = time.duration_since(UNIX_EPOCH).unwrap().as_secs();
    let days_since_epoch = datetime / (60 * 60 * 24);

    // Simplified date calculation
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let month_idx = ((days_since_epoch / 30) % 12) as usize;
    let day = (days_since_epoch % 30) + 1;

    let hour = (datetime / 3600) % 24;
    let minute = (datetime / 60) % 60;

    if now - timestamp > six_months {
        let year = 1970 + (days_since_epoch / 365);
        format!("{} {:2}  {:4}", months[month_idx], day, year)
    } else {
        format!("{} {:2} {:02}:{:02}", months[month_idx], day, hour, minute)
    }
}
