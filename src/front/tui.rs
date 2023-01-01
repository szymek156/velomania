//! Smallest possible UI, uses termion, for more fancy stuff2 tui.rs can be used

use std::{
    fmt::format,
    io::{stdout, Write},
};

use termion::raw::IntoRawMode;
use tokio::sync::broadcast::Receiver;

use crate::{indoor_bike_data_defs::BikeData, workout_state::WorkoutState};

pub async fn show(
    mut workout_rx: Receiver<WorkoutState>,
    indoor_bike_notif: Option<Receiver<BikeData>>,
    training_notif: Option<Receiver<String>>,
) {
    clear_all();

    if let (Some(mut indoor_bike_notif), Some(mut training_notif)) =
        (indoor_bike_notif, training_notif)
    {
        loop {
            tokio::select! {
                state = workout_rx.recv() => {
                    handle_workout_state(state.unwrap());
                }
                bike_data = indoor_bike_notif.recv() => {
                    handle_bike_data(bike_data.unwrap());
                }
                training_data = training_notif.recv() => {
                    handle_training_data(training_data.unwrap());
                }
            }
        }
    } else {
        loop {
            tokio::select! {
                state = workout_rx.recv() => {
                    handle_workout_state(state.unwrap());
                }
            }
        }
    }
}

fn handle_workout_state(state: WorkoutState) {
    let start_row = 1;
    let nr_lines = 9;
    clear(start_row, start_row + nr_lines);

    let data_str =
        format!("== WORKOUT STATE ==\n\rFTP base: {}\n\rcurrent power set: {}\n\rworkout duration: {}\n\rstep: {}/{}\n\rcurrent step: {:?}\n\rstep duration {}\n\rnext step: {:?}\n\r",
            state.ftp_base, state.current_power_set,
            humantime::format_duration(state.total_workout_duration),
            state.current_step_number,
            state.total_steps,
            state.current_step,
            humantime::format_duration(state.current_step_duration),
            state.next_step);

    let stdout = stdout();

    let mut stdout = stdout.lock().into_raw_mode().unwrap();

    write!(stdout, "{}{}", termion::cursor::Goto(1, start_row), data_str,).unwrap();
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
    let start_row = 10;
    let nr_lines = 11;
    clear(start_row, start_row + nr_lines);

    let data_str = format!("== BIKE DATA==\n\rTIME: {:?} --> {:?}\n\rDISTANCE {:?}\n\r\n\rPOWER {:?}\n\rSPEED{:?}\n\rCADENCE {:?}\n\rAVG POWER {:?}\n\rAVG SPEED {:?}\n\rAVG CADENCE {:?}\n\rRESISTANCE {:?}",
    data.elapsed_time, data.remaining_time, data.tot_distance, data.inst_power, data.inst_speed, data.inst_cadence, data.avg_power, data.avg_speed, data.avg_cadence, data.resistance_lvl);
    let stdout = stdout();

    let mut stdout = stdout.lock().into_raw_mode().unwrap();

    write!(stdout, "{}{}", termion::cursor::Goto(1, start_row), data_str,).unwrap();

    stdout.flush().unwrap();
}

/// Clear part of the screen
fn clear(start_row: u16, end_row: u16) {
    assert!(end_row >= start_row);

    let stdout = stdout();
    let mut stdout = stdout.lock().into_raw_mode().unwrap();

    for line in start_row..=end_row {
        write!(
            stdout,
            "{}{}",
            termion::cursor::Goto(1, line),
            termion::clear::CurrentLine,
        )
        .unwrap();
    }

    stdout.flush().unwrap();
}

fn clear_all() {
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
