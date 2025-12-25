pub enum Commands {
    User,
    Password,
    WorkingDir,
    ChangeDir,
    Features,
    System,
    Type,
    List,
    Port,
    Passive,
    Option,
    Quit,
    Unknown,
}

impl From<String> for Commands {
    fn from(val: String) -> Self {
        match val.as_str() {
            "USER" => Commands::User,
            "PASS" => Commands::Password,
            "PWD" | "XPWD" => Commands::WorkingDir,
            "CWD" => Commands::ChangeDir,
            "OPTS" => Commands::Option,
            "LIST" | "NLST" | "MLST" | "MLSD" => Commands::List,
            "PORT" => Commands::Port,
            "PASV" => Commands::Passive,
            "SYST" => Commands::System,
            "TYPE" => Commands::Type,
            "FEAT" => Commands::Features,
            "QUIT" => Commands::Quit,
            _ => Commands::Unknown,
        }
    }
}
