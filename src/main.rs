#[macro_use]
extern crate num_derive;
use std::{
    io,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use structopt::StructOpt;
use workout_state::WorkoutState;
use zwo_workout::ZwoWorkout;

use crate::ble_client::BleClient;
use anyhow::Result;
use cli::{UserCommands, WorkoutCommands};
use futures::StreamExt;
use indoor_bike_client::IndoorBikeFitnessMachine;
use indoor_bike_data_defs::ControlPointResult;
use signal_hook::consts::signal::*;
use signal_hook_async_std::Signals;
use tokio::{sync::broadcast::Receiver, task, time::Instant};

mod bk_gatts_service;
mod ble_client;
mod cli;
mod common;
mod front;
mod indoor_bike_client;
mod indoor_bike_data_defs;
mod scalar_converter;
mod workout_state;
mod zwo_workout;
mod zwo_workout_file;

#[macro_use]
extern crate log;

#[derive(StructOpt)]
struct Args {
    /// Workout .zwo file
    #[structopt(short, long, parse(from_os_str))]
    workout: PathBuf,

    #[structopt(short, long)]
    ftp_base: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let connect_to_trainer = true;

    let opt = Args::from_args();

    // Channel used by workout task to broadcast power value to be set - received by control_fit_machine, but also by frontend
    let (trainer_commands_tx, _command_rx) = tokio::sync::broadcast::channel(16);
    let (workout_state_tx, _rx) = tokio::sync::broadcast::channel(16);

    // Channel used to control workout, skip step, pause
    let (control_workout_tx, control_workout_rx) = tokio::sync::mpsc::channel(16);

    register_signal_handler(trainer_commands_tx.clone());


    let (fit, bike_notifications, training_notifications) = {
        if connect_to_trainer {
            let fit = connect_to_fit().await?;
            let bike_notifications = fit.subscribe_for_indoor_bike_notifications();
            let training_notifications = fit.subscribe_for_training_notifications();

            (
                Some(fit),
                Some(bike_notifications),
                Some(training_notifications),
            )
        } else {
            // TODO: create fake data in the future
            (None, None, None)
        }
    };

    // Start workout task, will broadcast next steps
    let workout_join_handle = start_workout(
        trainer_commands_tx.clone(),
        workout_state_tx.clone(),
        control_workout_rx,
        opt.workout.as_path(),
        opt.ftp_base,
    )
    .await?;

    handle_user_input(control_workout_tx);

    // Tui shows current step + data from trainer
    let tui_join_handle = tokio::spawn(front::tui::show(
        workout_state_tx.subscribe(),
        bike_notifications,
        training_notifications,
    ));

    if let Some(fit) = fit {
        control_fit_machine(fit, trainer_commands_tx.subscribe()).await?;
    } else {
        // Listen for sigterm
        let mut rx = trainer_commands_tx.subscribe();
        while let Ok(message) = rx.recv().await {
            match message {
                UserCommands::Exit => {
                    info!("Exit!");
                    break;
                }
                _ => (),
            }
        }
    };

    workout_join_handle.abort();
    tui_join_handle.abort();

    Ok(())
}

/// Reads ZWO file, and sends commands according to it
async fn start_workout(
    trainer_commands_tx: tokio::sync::broadcast::Sender<UserCommands>,
    workout_state_tx: tokio::sync::broadcast::Sender<WorkoutState>,
    mut control_workout_rx: tokio::sync::mpsc::Receiver<WorkoutCommands>,
    workout: &Path,
    ftp_base: f64,
) -> Result<tokio::task::JoinHandle<()>> {
    let mut workout = ZwoWorkout::new(&workout, ftp_base).await?;

    let handle = tokio::spawn(async move {
        debug!("spawning workout task");

        let propagate_workout_state = tokio::time::interval(Duration::from_secs(1));
        tokio::pin!(propagate_workout_state);

        loop {
            tokio::select! {
                workout_step = workout.next() => {
                    // Next step is available
                    match workout_step {
                        Some(command) => {
                            debug!("Got command from workout: {command:?}");
                            debug!("workout {}/{}",
                                workout.workout_state.current_step_number,
                                workout.workout_state.total_steps);

                            debug!("workout {:?}", workout.current_step);
                            trainer_commands_tx.send(command).unwrap();
                        }
                        None => {
                            debug!("No more steps in workout, workout task exits");
                            trainer_commands_tx.send(UserCommands::Exit).unwrap();
                            break;
                        },
                    }
                }
                // TODO: this is a workaround, ideally there would be:
                //
                // Some(workout_state) = workout.workout_state.next() => {
                //     workout_state_tx.send(workout.workout_state.clone()).unwrap();
                // }
                // But BC complains that mut borrow is already held on workout,
                // figure something out here
                // TODO: Arc? Gets immutable borrow Nope, RefCell? Nope will panic during runtime
                // Mutex? Will deadlock
                // Do subscribe to the channel from the workout state?
                // Move workout state as separate actor, let workout communicate to state via channel
                // to update it
                _ = propagate_workout_state.tick() => {
                    debug!("Broadcast workout state {}/{}",
                        workout.workout_state.current_step_number,
                        workout.workout_state.total_steps);

                    workout.workout_state.update_ts();
                    workout_state_tx.send(workout.workout_state.clone()).unwrap();
                }
                Some(control)  = control_workout_rx.recv() => {
                    match control {
                        WorkoutCommands::Pause=>workout.pause(),
                        WorkoutCommands::Resume=>todo!(),
                        WorkoutCommands::SkipStep=>workout.skip_step(),
                        WorkoutCommands::Abort => {
                            trainer_commands_tx.send(UserCommands::Exit).unwrap();
                            break;
                        },
                    }
                }
            }
        }
    });

    Ok(handle)
}

/// Gets the commands (may be ZWO workout, or user input), and passes them to the fitness machine
async fn control_fit_machine(
    fit: IndoorBikeFitnessMachine,
    mut rx: Receiver<UserCommands>,
) -> Result<()> {
    // Cannot set return type of async block, async closures are unstable

    fit.dump_service_info().await?;
    fit.get_features().await?;

    // TODO: Use select?
    // let _status_notifications = fit.subscribe_for_status_notifications();

    let mut cp_notifications = fit.subscribe_for_control_point_notifications();

    while let Ok(message) = rx.recv().await {
        match message {
            UserCommands::Exit => {
                info!("Control task exits");
                break;
            }
            UserCommands::SetResistance { resistance } => {
                fit.set_resistance(resistance).await?;
            }
            UserCommands::SetTargetPower { power } => {
                fit.set_power(power).await?;
            }
        }

        // Wait for CP notification response for above write request
        let resp = cp_notifications.recv().await?;
        match resp.request_status {
            ControlPointResult::Success => {
                debug!("Got ACK for request {resp:?}");
            }
            _ => {
                error!("Received NACK for request: {resp:?}");
            }
        }
    }

    fit.disconnect().await?;

    Ok(())
}

fn register_signal_handler(tx: tokio::sync::broadcast::Sender<UserCommands>) -> () {
    task::spawn(async move {
        info!("Signal handler waits for events");

        let mut signals = Signals::new(&[SIGINT]).unwrap();

        match signals.next().await {
            Some(sig) => {
                warn!("Got signal {sig}");
                tx.send(UserCommands::Exit).unwrap();
            }
            None => unreachable!("Signals stream closed?"),
        }
    });
}

async fn connect_to_fit() -> Result<IndoorBikeFitnessMachine> {
    let ble = BleClient::new().await;
    // ble.connect_to_bc().await.unwrap();

    let fit = IndoorBikeFitnessMachine::new(&ble).await?;

    Ok(fit)
}

pub fn handle_user_input(tx: tokio::sync::mpsc::Sender<WorkoutCommands>) {
    // It's not recommended to handle user input using async.
    // Spawn dedicated thread instead.

    // dropped join handle detaches thread
    thread::spawn(move || {
        info!("Waiting for user input");
        loop {
            let mut buffer = String::new();
            let res = io::stdin().read_line(&mut buffer);

            if let Err(e) = res {
                error!("Got error while reading stdin {e}, exiting");
                tx.blocking_send(WorkoutCommands::Abort).unwrap();
                break;
            }

            let input = buffer.trim().to_ascii_uppercase();

            match input.as_str() {
                "S" => {
                    tx.blocking_send(WorkoutCommands::SkipStep).unwrap();
                }
                "Q" => {
                    tx.blocking_send(WorkoutCommands::Abort).unwrap();
                }
                other @ _ => {
                    warn!("Unexpected user input {other}");
                }
            }
        }
    });
}
