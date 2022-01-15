#![feature(format_args_capture)]

use crate::ble_client::BleClient;

mod ble_client;

#[macro_use]
extern crate log;
#[tokio::main]
async fn main() {
    env_logger::init();

    let mut ble = BleClient::new().await;

    ble.connect_to_bc().await.unwrap();
}
