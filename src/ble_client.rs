use anyhow::Result;
use btleplug::api::bleuuid::BleUuid;
use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Manager, Peripheral, PeripheralId};
use futures::stream::StreamExt;

pub struct BleClient {
    adapter: Adapter,
    bc_client: Option<Peripheral>,
}

impl BleClient {
    pub async fn new() -> Self {
        let manager = Manager::new().await.unwrap();
        let adapters = manager.adapters().await.unwrap();

        // Get first adapter
        let adapter = adapters.into_iter().nth(0).unwrap();

        Self {
            adapter,
            bc_client: None,
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
                }
                CentralEvent::DeviceDisconnected(id) => {
                    println!("DeviceDisconnected: {:?}", id);
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

    pub async fn list_bc_files(&self) -> Result<()> {
        if let Some(bc) = &self.bc_client {
            debug!("services listing");
            bc.discover_services().await?;

            for service in &bc.services() {
                info!(
                    "Service UUID {}, primary: {}",
                    service.uuid, service.primary
                );
                for characteristic in &service.characteristics {
                    info!("  {:?}", characteristic);
                }
            }
        }

        debug!("listing end");

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

        debug!("DeviceDiscovered: {local_name}, connected {is_connected}");

        // TODO: comparing UUID would be more robust
        if local_name == "BK_GATTS" && !is_connected {
            info!("Connecting to {local_name}");
            peripheral.connect().await?;

            self.bc_client = Some(peripheral);

            // TODO: to be removed
            self.list_bc_files().await?;
        }

        Ok(())
    }
}
