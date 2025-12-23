use crate::config::Config;

pub struct Server {
    config: Config,
}

impl Server {
    pub fn new(config: Config) -> Self {
        Server { config }
    }

    pub fn start_server(&self) -> Result<()> {}
}
