use std::sync::Arc;

use anyhow::{Result, anyhow};
use tokio::net::TcpListener;
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, fmt};

use crate::{
    config::Config,
    session::{ConnectionError, Session},
};

pub struct Server {
    config: Config,
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_level(true)
        .compact()
        .init();
}

impl Server {
    pub fn new(config: Config) -> Self {
        Server { config }
    }

    pub async fn start_server(&self) -> Result<()> {
        init_logging();
        info!("Dock FTP Server {}", env!("CARGO_PKG_VERSION"));
        let listener = TcpListener::bind(&self.config.address)
            .await
            .map_err(|_| anyhow!("failed to bind to given address"))?;
        info!("Listening on {}", self.config.address);

        let arc_config = Arc::new(self.config.clone());

        loop {
            let (socket, addr) = listener
                .accept()
                .await
                .map_err(|_| anyhow!("cannot accept connection"))?;

            info!(ip=%addr, "Got new connection.");
            let arc_config_cloned = Arc::clone(&arc_config);

            tokio::spawn(async move {
                let session_id = cuid2::cuid();
                let mut session = Session::new(&session_id, socket, (*arc_config_cloned).clone());
                info!(session_id=%session_id, ip=%addr, "Initiated new session.");
                if let Err(e) = session.run_session().await {
                    if e == ConnectionError::ClosedByQuit {
                        info!(session_id=%session_id, "Session was closed by user.");
                    } else {
                        error!(session_id=%session_id, reason=%e, "Session failed");
                    }
                }
            });
        }
    }
}
