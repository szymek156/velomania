//! Smallest possible UI, uses termion, for more fancy stuff2 tui.rs can be used

use std::{
    fmt::format,
    io::{stdout, Write},
};

use termion::raw::IntoRawMode;
use tokio::sync::broadcast::Receiver;

use crate::{cli::UserCommands, indoor_bike_data_defs::BikeData};

pub async fn show(
    mut workout_rx: Receiver<UserCommands>,
    mut indoor_bike_notif: Receiver<BikeData>,
    mut training_notif: Receiver<String>,
) {
    clear();
    loop {
        tokio::select! {
            c = workout_rx.recv() => {
                let c = c.unwrap();

                match c {
                    UserCommands::Exit => {
                        let stdout = stdout();

                        let mut stdout = stdout.lock().into_raw_mode().unwrap();
                        let _ = write!(
                            stdout,
                            "{}{} Got exit!{}",
                            termion::cursor::Goto(1, 11),
                            termion::clear::CurrentLine,
                            termion::cursor::Goto(1, 1),
                        );
                        break
                    }
                    other @ _  => {
                        handle_workout_step(other);
                    },
                }
            }
            bike_data = indoor_bike_notif.recv() => {
                handle_bike_data(bike_data.unwrap());
            }
            training_data = training_notif.recv() => {
                handle_training_data(training_data.unwrap());
            }
        }
    }
}

fn handle_workout_step(c: UserCommands) {
    let stdout = stdout();

    let mut stdout = stdout.lock().into_raw_mode().unwrap();

    write!(
        stdout,
        "{}{} Workout step: {:?}{}",
        termion::cursor::Goto(1, 51),
        termion::clear::CurrentLine,
        c,
        termion::cursor::Goto(1, 1),
    )
    .unwrap();

    stdout.flush().unwrap();
}

fn handle_training_data(data: String) {
    let stdout = stdout();

    let mut stdout = stdout.lock().into_raw_mode().unwrap();

    write!(
        stdout,
        "{}{} Training Data: {}{}",
        termion::cursor::Goto(1, 21),
        termion::clear::BeforeCursor,
        data,
        termion::cursor::Goto(1, 1),
    )
    .unwrap();

    stdout.flush().unwrap();
}

fn handle_bike_data(data: BikeData) {
    let data_str = format!("TIME: {:?} --> {:?}\n\rDISTANCE {:?}\n\r\n\rPOWER {:?}\n\rSPEED{:?}\n\rCADENCE {:?}\n\rAVG POWER {:?}\n\rAVG SPEED {:?}\n\rAVG CADENCE {:?}\n\rRESISTANCE {:?}",
    data.elapsed_time, data.remaining_time, data.tot_distance, data.inst_power, data.inst_speed, data.inst_cadence, data.avg_power, data.avg_speed, data.avg_cadence, data.resistance_lvl);
    let stdout = stdout();

    let mut stdout = stdout.lock().into_raw_mode().unwrap();

    write!(
        stdout,
        "{}{}{}{}",
        termion::cursor::Goto(1, 31),
        termion::clear::BeforeCursor,
        data_str,
        termion::cursor::Goto(1, 1),
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
        termion::cursor::Goto(1, 1),
        termion::clear::All,
    )
    .unwrap();

    stdout.flush().unwrap();
}
