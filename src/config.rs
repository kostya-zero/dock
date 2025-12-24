use std::{collections::HashMap, fs};

use anyhow::{Result, anyhow};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub address: String,
    pub users: HashMap<String, String>,
    pub root: String,
}

#[derive(Debug)]
pub enum ConfigError {
    UserNotFound,
    WrongPasswor,
}

impl Config {
    pub fn check_user(&self, username: &String) -> bool {
        self.users.contains_key(username)
    }

    pub fn check_password(&self, username: &String, password: &String) -> bool {
        if let Some(pass) = self.users.get(username) {
            pass == password
        } else {
            false
        }
    }
}

pub fn load_config(path: &str) -> Result<Config> {
    let content = fs::read_to_string(path).map_err(|_| anyhow!("a file system error occurred."))?;
    let config =
        serde_json::from_str::<Config>(&content).map_err(|e| anyhow!("bad config format: {e}"))?;
    Ok(config)
}
