use anyhow::Result;
use btleplug::api::bleuuid::{BleUuid, uuid_from_u16};
use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Manager, Peripheral, PeripheralId};
use futures::stream::StreamExt;
use uuid::Uuid;

use crate::bk_gatts_service::{self, BkClient};

pub struct BleClient {
    adapter: Adapter,
    // TODO: peripheral should be send via channel, no kept inside BleClient struct
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

    /// Scans over devices, attempts to connect, looks for given service
    /// Returns peripheral of first found device that has requested service
    pub async fn find_service(&self, gatts_service: Uuid) -> Result<Option<Peripheral>> {
        // TODO: probably it's enough to use ScanFilter with the uuid
        let speed_cadence = uuid_from_u16(0x1816);
        let power = uuid_from_u16(0x1818);

        self.adapter
            .start_scan(ScanFilter {
                services: vec![gatts_service, speed_cadence, power],
            })
            .await?;

        info!("Started scanning for devices...");

        let mut events = self.adapter.events().await?;

        // Print based on whatever the event receiver outputs. Note that the event
        // receiver blocks, so in a real program, this should be run in its own
        // thread (not task, as this library does not yet use async channels).

        // Instead of bool flags, do a state machine
        let mut connection_successful = false;
        let mut connected_device = "Not set".to_string();
        while let Some(event) = events.next().await {
            match event {
                CentralEvent::DeviceDiscovered(id) => {
                    if connection_successful {
                        continue;
                    }

                    let peripheral = self.adapter.peripheral(&id).await?;

                    let properties = peripheral.properties().await?;
                    let is_connected = peripheral.is_connected().await?;
                    let local_name = properties
                        .unwrap()
                        .local_name
                        .unwrap_or(String::from("(peripheral name unknown)"));

                    debug!("DeviceDiscovered: {local_name} {id:?}, connected {is_connected}");

                    // TODO: to speedup the process...
                    // TODO: comparing UUID would be more robust
                    if local_name != "SUITO" {
                        continue;
                    }

                    info!("Connecting to {local_name}...");
                    // TODO: how to setup a reasonable timeout?
                    if let Err(e) = peripheral.connect().await {
                        warn!("Connection failed {e}");
                        continue;
                    } else {
                        info!("Connected!");
                        connection_successful = true;
                        connected_device = local_name.to_string();
                    }
                }
                CentralEvent::DeviceConnected(id) => {
                    println!("DeviceConnected: {:?}", id);
                    let peripheral = self.adapter.peripheral(&id).await?;

                    peripheral.discover_services().await?;

                    let found = peripheral
                        .services()
                        .into_iter()
                        .find(|service| service.uuid == gatts_service);

                    if found.is_some() {
                        return Ok(Some(peripheral));
                    } else {
                        let local_name = connected_device;
                        warn!("{local_name} Does not have requested service, disconnecting");

                        // TODO: this disconnects unrelated BT devices, like headphones :D
                        peripheral.disconnect().await?;
                        connection_successful = false;
                        connected_device = "Not set".to_string();
                    }
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
                CentralEvent::DeviceUpdated(id) => warn!("Got DeviceUpdated event for {id:?}"),
            }
        }

        Ok(None)
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
                CentralEvent::DeviceUpdated(id) => warn!("Got DeviceUpdated event for {id:?}"),
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

                bk.fetch_file(&files[1]).await?;
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
