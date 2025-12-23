use clap::Parser;

#[derive(Parser)]
#[command(
    name = "dock",
    arg_required_else_help = false,
    subcommand_required = false
)]
pub struct Cli {
    /// The path to the configuration file.
    #[arg(short, long)]
    pub config: Option<String>,
}
