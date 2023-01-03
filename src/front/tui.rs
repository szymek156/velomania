//! Smallest possible UI, uses termion, for more fancy stuff2 tui.rs can be used

use std::{
    fmt::format,
    io::{stdout, Write},
    time::Duration,
};

use termion::raw::IntoRawMode;
use tokio::sync::broadcast::Receiver;

use crate::{
    common::{duration_to_string, get_power},
    indoor_bike_data_defs::BikeData,
    workout_state::{IntervalState, WorkoutState},
    zwo_workout_file::WorkoutSteps,
};

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
        format!("== WORKOUT STATE ==\n\rFTP base: {}\n\rcurrent power set: {}W\n\rworkout duration: {} elapsed {} to go {}\n\rstep: {}/{}\n\rcurrent step: {}\n\rstep duration {} elapsed {} to go {}\n\r{}next step: {}\n\r",
            state.ftp_base, state.current_power_set,
            duration_to_string(&state.total_workout_duration),
            duration_to_string(&state.workout_elapsed),
            duration_to_string(&state.total_workout_duration.saturating_sub(state.workout_elapsed)),
            state.current_step_number,
            state.total_steps,
            display_step(state.ftp_base, &Some(state.current_step.step)),
            duration_to_string(&state.current_step.duration),
            duration_to_string(&state.current_step.elapsed),
            duration_to_string(&state.current_step.duration.saturating_sub(state.current_step.elapsed)),
            display_interval(&state.current_interval),
            display_step(state.ftp_base, &state.next_step));

    let stdout = stdout();

    let mut stdout = stdout.lock().into_raw_mode().unwrap();

    write!(
        stdout,
        "{}{}",
        termion::cursor::Goto(1, start_row),
        data_str,
    )
    .unwrap();
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

    write!(
        stdout,
        "{}{}",
        termion::cursor::Goto(1, start_row),
        data_str,
    )
    .unwrap();

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

pub fn display_step(ftp_base: f64, step: &Option<WorkoutSteps>) -> String {
    if let Some(step) = step {
        match step {
            WorkoutSteps::Warmup(s) => format!(
                "Warmup: {}W -> {}W",
                get_power(ftp_base, s.power_low),
                get_power(ftp_base, s.power_high)
            ),
            WorkoutSteps::Ramp(s) => format!(
                "Ramp: {}W -> {}W",
                get_power(ftp_base, s.power_low),
                get_power(ftp_base, s.power_high)
            ),
            WorkoutSteps::SteadyState(s) => {
                format!("Steady State: {}W", get_power(ftp_base, s.power))
            }
            WorkoutSteps::Cooldown(s) => format!(
                "Cool down: {}W -> {}W",
                get_power(ftp_base, s.power_low),
                get_power(ftp_base, s.power_high)
            ),
            WorkoutSteps::IntervalsT(s) => format!(
                "Intervals: repeat {}, work {}W for {}, rest {}W for {}",
                s.repeat,
                get_power(ftp_base, s.on_power),
                duration_to_string(&Duration::from_secs(s.on_duration)),
                get_power(ftp_base, s.off_power),
                duration_to_string(&Duration::from_secs(s.off_duration))
            ),
            WorkoutSteps::FreeRide(_) => format!("Free Ride",),
        }
    } else {
        "None".to_string()
    }
}

pub fn display_interval(interval: &Option<IntervalState>) -> String {
    if let Some(interval) = interval {
        let interval_type = if interval.is_work_interval {
            "WORK"
        } else {
            "REST"
        };

        format!(
            "interval #{} {} elapsed {}, to go {}\n\r",
            interval.repetition,
            interval_type,
            duration_to_string(&interval.elapsed),
            duration_to_string(&interval.duration.saturating_sub(interval.elapsed))
        )
    } else {
        "".to_string()
    }
}
