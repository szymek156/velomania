use clap::{ErrorKind, FromArgMatches, IntoApp, Parser, Subcommand};
use std::{
    io::{self},
    thread,
};
use tokio::sync::mpsc::Sender;

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    command: UserCommands,
}

/// Things possible to control from the CLI
#[derive(Debug, Subcommand, Clone)]
pub enum UserCommands {
    // Use clap to model possible commands
    // User can type help to get description, for free!

    /// Prepare machine for new workout
    StartWorkout,
    SetResistance{resistance : u8},

    SetTargetPower{power: i16},
    /// Exits the application
    Exit,
}

/// Commands to control flow of the workout
#[derive(Debug)]
pub enum WorkoutCommands {
    Pause,
    Resume,
    SkipStep,
    Abort
}

/// Read stdin and use clap to parse user input to the CLIMessages enum
pub fn control_cli(tx: Sender<UserCommands>) {
    // It's not recommended to handle user input using async.
    // Spawn dedicated thread instead.

    thread::spawn(move || {
        info!("Waiting for user input");
        loop {
            let mut buffer = String::new();
            let res = io::stdin().read_line(&mut buffer);

            if let Err(e) = res {
                error!("Got error while reading stdin {e}, exiting");
                tx.blocking_send(UserCommands::Exit).unwrap();
                break;
            }

            let matches = Cli::command()
                .no_binary_name(true)
                .try_get_matches_from(buffer.trim().split(' '));

            match matches {
                Ok(matches) => {
                    // Matches are valid, so it's safe to unwrap
                    let cli = UserCommands::from_arg_matches(&matches).unwrap();

                    tx.blocking_send(cli).unwrap();
                }
                Err(e) => match e.kind() {
                    // DisplayHelp is not an error, so print it on info level
                    ErrorKind::DisplayHelp => {
                        // TODO: format it to fit better for interactive CLI
                        info!("\n{e}");
                    }
                    _ => {
                        error!("Invalid command! Type 'help'");
                        error!("{e}");
                    }
                },
            }
        }
    });
}
