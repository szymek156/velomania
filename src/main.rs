use anyhow::Result;
use cli::{control_cli, CLIMessages};
use fitness_machine_client::FitnessMachine;

use crate::ble_client::BleClient;

mod bk_gatts_service;
mod ble_client;
mod cli;
mod fitness_machine_client;

#[macro_use]
extern crate log;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let fit: Option<FitnessMachine> = None; //Some(connect_fot_fit().await?);

    let (tx, mut rx) = tokio::sync::mpsc::channel(10);

    control_cli(tx).await;

    while let Some(m) = rx.recv().await {
        match m {
            CLIMessages::Exit => {
                rx.close();
                break;
            }
            _ => unimplemented!(),
        }
    }

    if let Some(fit) = fit {
        fit.disconnect().await
    } else {
        Ok(())
    }
}

async fn connect_fot_fit() -> Result<FitnessMachine> {
    let ble = BleClient::new().await;
    // ble.connect_to_bc().await.unwrap();

    let mut fit = FitnessMachine::default();

    fit.connect_to_service(&ble).await?;

    Ok(fit)
}
