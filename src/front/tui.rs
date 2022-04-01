//! Smallest possible UI, uses termion, for more fancy stuff2 tui.rs can be used

use std::io::{stdout, Write};

use termion::raw::IntoRawMode;

use crate::cli::UserCommands;

pub async fn test(mut workout_rx: tokio::sync::broadcast::Receiver<UserCommands>) {
    clear();
    loop {
        tokio::select! {
            c = workout_rx.recv() => {
                let c = c.unwrap();

                match c {
                    UserCommands::Exit => break,
                    other @ _  => {
                        handle_workout_step(other);
                    },
                }
            }
        }
    }
}

fn handle_workout_step(c: UserCommands) {
    let stdout = stdout();

    let mut stdout = stdout.lock().into_raw_mode().unwrap();

    write!(
        stdout,
        "{}{} Workout step: {:?}",
        termion::cursor::Goto(1, 11),
        termion::clear::CurrentLine,
        c
    )
    .unwrap();

    stdout.flush().unwrap();
}

fn clear() {
    let stdout = stdout();
    let mut stdout = stdout.lock().into_raw_mode().unwrap();

    write!(
        stdout,
        "{}{}",
        termion::clear::All,
        termion::cursor::Goto(1, 1),
    )
    .unwrap();

    stdout.flush().unwrap();
}
