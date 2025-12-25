use std::{collections::HashMap, fs};

use anyhow::{Result, anyhow};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub enum Permissions {
    Write,
    Read,
    All,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Config {
    pub address: String,
    pub users: Vec<User>,
    pub root: String,
    #[serde(skip, default)]
    pub users_map: HashMap<String, User>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct User {
    pub name: String,
    pub password: String,
    pub permissions: Permissions,
}

#[derive(Debug)]
pub enum ConfigError {
    UserNotFound,
    WrongPassword,
}

impl Config {
    /// Checks if user exists.
    pub fn check_user(&self, username: &str) -> bool {
        if !self.users_map.is_empty() {
            self.users_map.contains_key(username)
        } else {
            self.users.iter().any(|f| f.name == username)
        }
    }

    // Checks if user's password matches.
    pub fn check_password(&self, username: &str, password: &str) -> bool {
        if !self.users_map.is_empty() {
            self.users_map
                .get(username)
                .map(|u| u.password == password)
                .unwrap_or(false)
        } else {
            self.users
                .iter()
                .any(|f| f.name == username && f.password == password)
        }
    }

    /// Checks if user has access to write.
    pub fn can_user_write(&self, username: &str) -> bool {
        if let Some(user) = self.users_map.get(username) {
            user.permissions == Permissions::Write || user.permissions == Permissions::All
        } else {
            false
        }
    }

    /// Checks if user has access to read.
    pub fn can_user_read(&self, username: &str) -> bool {
        if let Some(user) = self.users_map.get(username) {
            user.permissions == Permissions::Read || user.permissions == Permissions::All
        } else {
            false
        }
    }
}

pub fn load_config(path: &str) -> Result<Config> {
    let content = fs::read_to_string(path).map_err(|_| anyhow!("a file system error occurred."))?;
    let mut config =
        serde_json::from_str::<Config>(&content).map_err(|e| anyhow!("bad config format: {e}"))?;
    config.users_map = config
        .users
        .iter()
        .cloned()
        .map(|u| (u.name.clone(), u))
        .collect();
    Ok(config)
}
