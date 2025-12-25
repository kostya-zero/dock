use std::sync::Arc;

use anyhow::{Result, anyhow};
use tokio::net::TcpListener;
use tracing::info;

use crate::{
    config::Config,
    session::{ConnectionError, Session},
};

pub struct Server {
    config: Config,
}

impl Server {
    pub fn new(config: Config) -> Self {
        Server { config }
    }

    pub async fn start_server(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.config.address)
            .await
            .map_err(|_| anyhow!("failed to bind to given address"))?;
        info!("Listening on {}", self.config.address);
        println!("Starting dock");

        let arc_config = Arc::new(self.config.clone());

        loop {
            let (socket, addr) = listener
                .accept()
                .await
                .map_err(|_| anyhow!("cannot accept connection"))?;

            info!("Got new connection: {addr}");
            let arc_config_cloned = Arc::clone(&arc_config);

            tokio::spawn(async move {
                let session_id = cuid2::cuid();
                let mut session =
                    Session::new(&session_id, addr, socket, (*arc_config_cloned).clone());
                if let Err(e) = session.run_session().await {
                    if e == ConnectionError::ClosedByQuit {
                        println!("session was closed by user ({session_id})");
                    } else {
                        eprintln!("session failed ({session_id}): {e}");
                    }
                }
            });
        }
    }
}
