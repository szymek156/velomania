#[macro_use]
extern crate num_derive;
use anyhow::anyhow;

use crate::ble_client::BleClient;
use anyhow::Result;
use cli::{control_cli, UserCommands};
use futures::StreamExt;
use indoor_bike_client::IndoorBikeFitnessMachine;
use indoor_bike_data_defs::{ControlPointOpCode, ControlPointResult};
use signal_hook::consts::signal::*;
use signal_hook_async_std::Signals;
use tokio::{sync::mpsc::Receiver, task};

mod bk_gatts_service;
mod ble_client;
mod cli;
mod indoor_bike_client;
mod indoor_bike_data_defs;
mod scalar_converter;

#[macro_use]
extern crate log;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let (tx, rx) = tokio::sync::mpsc::channel(10);

    control_cli(tx.clone()).await;

    register_signal_handler(tx.clone());

    let mut fit = connect_fot_fit().await?;

    let res = run(&mut fit, rx).await;

    if res.is_err() {
        error!("Got error {}", res.as_ref().unwrap_err());
    }

    fit.disconnect().await?;

    res
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

fn register_signal_handler(tx: tokio::sync::mpsc::Sender<UserCommands>) -> () {
    task::spawn(async move {
        info!("Signal handler waits for events");

        let mut signals = Signals::new(&[SIGINT]).unwrap();

        match signals.next().await {
            Some(sig) => {
                warn!("Got signal {sig}");
                tx.send(UserCommands::Exit).await.unwrap();
            }
            None => unreachable!("Signals stream closed?"),
        }
    });
}

async fn connect_fot_fit() -> Result<IndoorBikeFitnessMachine> {
    let ble = BleClient::new().await;
    // ble.connect_to_bc().await.unwrap();

    let fit = IndoorBikeFitnessMachine::new(&ble).await?;

    Ok(fit)
}
