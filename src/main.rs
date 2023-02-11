use clap::{Parser, Subcommand};

mod content;
mod errors;
mod pack;
use pack::PackCommand;
mod serve;
use serve::ServeCommand;
mod templates;

fn main() {
    let cli = Cli::parse();
    let command: Box<dyn CommandRun> = cli.command.into();
    command.run();
}

#[derive(Parser)]
#[command(name = "wrustblog")]
#[command(author = "Manel Montilla")]
#[command(version = "0.0.1")]
#[command(about = "Simple blog engine")]
#[command(long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generates a directory with the given blog contents.
    Pack(PackCommand),
    /// Dynamically serves the contents of the blog.
    Serve(ServeCommand),
}

impl From<Commands> for Box<dyn CommandRun> {
    fn from(command: Commands) -> Self {
        match command {
            Commands::Pack(command) => Box::new(command),
            Commands::Serve(command) => Box::new(command),
        }
    }
}

trait CommandRun {
    fn run(&self);
}
