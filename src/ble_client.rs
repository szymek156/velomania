use anyhow::Result;
use btleplug::api::bleuuid::BleUuid;
use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Manager, PeripheralId};
use futures::stream::StreamExt;

use crate::bk_gatts_service::{self, BkClient};

pub struct BleClient {
    adapter: Adapter,
    // TODO: peripherial should be send via channel, no kept inside BleClient struct
    // fix it... someday
    bk_client: Option<BkClient>,
}

// TODO: handle device disconnect

impl BleClient {
    pub async fn new() -> Self {
        let manager = Manager::new().await.unwrap();
        let adapters = manager.adapters().await.unwrap();

        // Get first adapter
        let adapter = adapters.into_iter().nth(0).unwrap();

        Self {
            adapter,
            bk_client: None,
        }
    }

    /// Currently this function is only for testing purposes
    pub async fn connect_to_bc(&mut self) -> Result<()> {
        // start scanning for devices
        self.adapter.start_scan(ScanFilter::default()).await?;

        let mut events = self.adapter.events().await?;

        // Print based on whatever the event receiver outputs. Note that the event
        // receiver blocks, so in a real program, this should be run in its own
        // thread (not task, as this library does not yet use async channels).
        while let Some(event) = events.next().await {
            match event {
                CentralEvent::DeviceDiscovered(id) => {
                    self.device_discovered(&id).await?;
                }
                CentralEvent::DeviceConnected(id) => {
                    println!("DeviceConnected: {:?}", id);
                    self.device_connected(&id).await?;
                }
                CentralEvent::DeviceDisconnected(id) => {
                    println!("DeviceDisconnected: {:?}", id);
                    self.device_disconnected(&id).await?;
                }
                CentralEvent::ManufacturerDataAdvertisement {
                    id,
                    manufacturer_data,
                } => {
                    println!(
                        "ManufacturerDataAdvertisement: {:?}, {:?}",
                        id, manufacturer_data
                    );
                }
                CentralEvent::ServiceDataAdvertisement { id, service_data } => {
                    println!("ServiceDataAdvertisement: {:?}, {:?}", id, service_data);
                }
                CentralEvent::ServicesAdvertisement { id, services } => {
                    let services: Vec<String> =
                        services.into_iter().map(|s| s.to_short_string()).collect();
                    println!("ServicesAdvertisement: {:?}, {:?}", id, services);
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn device_discovered(&mut self, id: &PeripheralId) -> Result<()> {
        let peripheral = self.adapter.peripheral(id).await?;

        let properties = peripheral.properties().await?;
        let is_connected = peripheral.is_connected().await?;
        let local_name = properties
            .unwrap()
            .local_name
            .unwrap_or(String::from("(peripheral name unknown)"));

        debug!("DeviceDiscovered: {local_name} {id:?}, connected {is_connected}");

        // TODO: comparing UUID would be more robust
        if local_name == bk_gatts_service::SERVICE_NAME && !is_connected {
            info!("Connecting to {local_name}");
            peripheral.connect().await?;
            peripheral.discover_services().await?;

            self.bk_client = Some(BkClient { client: peripheral });
        }

        Ok(())
    }

    async fn device_connected(&self, id: &PeripheralId) -> Result<()> {
        if let Some(bk) = &self.bk_client {
            if &bk.client.id() == id {
                // TODO: to be removed
                let files = bk.list_bc_files().await?;
                info!("Files on the device {files:?}");

                bk.fetch_file(&files[0]).await?;
            }
        }

        Ok(())
    }

    async fn device_disconnected(&mut self, id: &PeripheralId) -> Result<()> {
        if let Some(bk) = &self.bk_client {
            if &bk.client.id() == id {
                // Drop current connection
                // self.bk_client.client.take();

                info!("BK disconnected, waiting for reconnect...");

                // self.adapter.stop_scan().await?;
                // self.adapter.start_scan(ScanFilter::default()).await?;
            }
        }

        Ok(())
    }
}
