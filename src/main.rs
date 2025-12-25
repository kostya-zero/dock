use std::process::exit;

use clap::Parser;
use dock::{cli::Cli, config::load_config, server::Server};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let config_path = cli.config.unwrap_or(String::from("config.json"));
    let config = match load_config(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to load configuration: {e}");
            exit(1);
        }
    };

    let server = Server::new(config);
    if let Err(e) = server.start_server().await {
        eprintln!("Server error occurred: {e}");
    }
}
