pub enum Commands {
    User,
    Password,
    Unknown,
}

impl From<String> for Commands {
    fn from(val: String) -> Self {
        match val.as_str() {
            "USER" => Commands::User,
            "PASS" => Commands::Password,
            _ => Commands::Unknown,
        }
    }
}
