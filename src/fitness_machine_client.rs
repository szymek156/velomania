use anyhow::anyhow;
use anyhow::Result;
use btleplug::api::bleuuid::uuid_from_u16;
use btleplug::api::Peripheral as _;
use btleplug::platform::Peripheral;

use uuid::Uuid;

use crate::ble_client::BleClient;

/// GATTS Service UUID
const SERVICE_UUID: Uuid = uuid_from_u16(0x1826);
/// Characteristic to retrieve supported features
/// Like cadence, power measurement, etc
const FEATURE: Uuid = uuid_from_u16(0x2ACC);

/// Characteristic to send control messages
const CONTROL_POINT: Uuid = uuid_from_u16(0x2AD9);

const STATUS: Uuid = uuid_from_u16(0x2ADA);

#[derive(Default)]
pub struct FitnessMachine {
    pub client: Option<Peripheral>,
}

impl FitnessMachine {
    pub async fn connect_to_service(&mut self, ble: &BleClient) -> Result<()> {
        let res = ble.find_service(SERVICE_UUID).await?;

        if res.is_some() {
            self.client = res;

            self.dump_service_info().await?;

            Ok(())
        } else {
            Err(anyhow!("Fitness machine device not found"))
        }
    }

    pub async fn dump_service_info(&self) -> Result<()> {
        let client = self.client.as_ref().unwrap();

        for service in client.services() {
            info!(
                "Service UUID {}, primary: {}",
                service.uuid, service.primary
            );
            for characteristic in service.characteristics {
                info!("  {:?}", characteristic);
            }
        }

        Ok(())
    }

    pub(crate) async fn disconnect(&self) -> Result<()> {
        let client = self.client.as_ref().unwrap();
        let name = client.properties().await?.unwrap().local_name.unwrap();
        info!("Disconnecting from {name}");
        client.disconnect().await?;

        Ok(())
    }
}
