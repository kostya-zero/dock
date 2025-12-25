pub enum Commands {
    User,
    Password,
    WorkingDir,
    ChangeDir,
    Features,
    System,
    Type,
    ChangeDirectoryUp,
    List,
    Port,
    Size,
    Retrive,
    Store,
    Rest,
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
            "CDUP" => Commands::ChangeDirectoryUp,
            "OPTS" => Commands::Option,
            "LIST" | "NLST" | "MLST" | "MLSD" => Commands::List,
            "PORT" => Commands::Port,
            "REST" => Commands::Rest,
            "PASV" => Commands::Passive,
            "RETR" => Commands::Retrive,
            "STOR" => Commands::Store,
            "SIZE" => Commands::Size,
            "SYST" => Commands::System,
            "TYPE" => Commands::Type,
            "FEAT" => Commands::Features,
            "QUIT" => Commands::Quit,
            _ => Commands::Unknown,
        }
    }
}
