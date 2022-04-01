#[macro_use]
extern crate num_derive;
use std::{
    path::{Path, PathBuf},
    thread::JoinHandle,
};

use structopt::StructOpt;
use zwo_workout::ZwoWorkout;

use crate::ble_client::BleClient;
use anyhow::Result;
use cli::{UserCommands};
use futures::StreamExt;
use indoor_bike_client::IndoorBikeFitnessMachine;
use indoor_bike_data_defs::ControlPointResult;
use signal_hook::consts::signal::*;
use signal_hook_async_std::Signals;
use tokio::{sync::mpsc::Receiver, task};

mod bk_gatts_service;
mod ble_client;
mod cli;
mod indoor_bike_client;
mod indoor_bike_data_defs;
mod scalar_converter;
mod zwo_workout;
mod zwo_workout_steps;
mod front;

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


    let opt = Args::from_args();

    let (command_tx, _rx) = tokio::sync::broadcast::channel(16);


    register_signal_handler(command_tx.clone());


    // let mut fit = connect_to_fit().await?;

    let handle = start_workout(command_tx.clone(), opt.workout.as_path(), opt.ftp_base).await?;

    front::tui::test(command_tx.subscribe()).await;

    let _ = handle.await;
    let res = Ok(());

    // let res = run(&mut fit, rx).await;

    // if res.is_err() {
    //     error!("Got error {}", res.as_ref().unwrap_err());
    // }

    // fit.disconnect().await?;

    res
}

async fn start_workout(
    tx: tokio::sync::broadcast::Sender<UserCommands>,
    workout: &Path,
    ftp_base: f64,
) -> Result<tokio::task::JoinHandle<()>> {
    let mut workout = ZwoWorkout::new(&workout, ftp_base).await?;

    let handle = tokio::spawn(async move {
        debug!("spawning workout stream");

        while let Some(command) = workout.next().await {
            // TODO: unwrap
            debug!("Got command from workout: {command:?}");

            tx.send(command).unwrap();
        }

        tx.send(UserCommands::Exit).unwrap();
    });

    Ok(handle)
}

async fn run(fit: &mut IndoorBikeFitnessMachine, mut rx: Receiver<UserCommands>) -> Result<()> {
    fit.dump_service_info().await?;
    fit.get_features().await?;

    // TODO: Use select?
    // let _status_notifications = fit.subscribe_for_status_notifications();

    let mut cp_notifications = fit.subscribe_for_control_point_notifications();

    while let Some(message) = rx.recv().await {
        match message {
            UserCommands::Exit => {
                rx.close();
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
