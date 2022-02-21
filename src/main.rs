#[macro_use]
extern crate num_derive;

use crate::ble_client::BleClient;
use anyhow::Result;
use cli::{control_cli, CLIMessages};
use fitness_machine_client::FitnessMachine;
use futures::StreamExt;
use signal_hook::consts::signal::*;
use signal_hook_async_std::Signals;
use tokio::{sync::mpsc::Receiver, task};

mod bk_gatts_service;
mod ble_client;
mod cli;
mod fitness_machine_client;

#[macro_use]
extern crate log;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let (tx, mut rx) = tokio::sync::mpsc::channel(10);

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

async fn run(fit: &mut FitnessMachine, mut rx: Receiver<CLIMessages>) -> Result<()> {
    fit.dump_service_info().await?;
    fit.get_features().await?;

    // Use select?
    let _status_notifications = fit.subscribe_for_status_notifications();

    while let Some(m) = rx.recv().await {
        match m {
            CLIMessages::Exit => {
                rx.close();
                break;
            }
            _ => unimplemented!(),
        }
    }

    Ok(())
}

fn register_signal_handler(tx: tokio::sync::mpsc::Sender<CLIMessages>) -> () {
    task::spawn(async move {
        info!("Signal handler waits for events");

        let mut signals = Signals::new(&[SIGINT]).unwrap();

        match signals.next().await {
            Some(sig) => {
                warn!("Got signal {sig}");
                tx.send(CLIMessages::Exit).await.unwrap();
            }
            None => unreachable!("Signals stream closed?"),
        }
    });
}

async fn connect_fot_fit() -> Result<FitnessMachine> {
    let ble = BleClient::new().await;
    // ble.connect_to_bc().await.unwrap();

    let fit = FitnessMachine::new(&ble).await?;

    Ok(fit)
}
