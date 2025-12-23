use std::{collections::HashMap, fs};

use anyhow::{Result, anyhow};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub address: String,
    pub users: HashMap<String, String>,
    pub root: String,
}

pub fn load_config(path: &str) -> Result<Config> {
    let content = fs::read_to_string(path).map_err(|_| anyhow!("a file system error occurred."))?;
    let config =
        serde_json::from_str::<Config>(&content).map_err(|e| anyhow!("bad config format: {e}"))?;
    Ok(config)
}
