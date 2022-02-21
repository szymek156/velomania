use std::vec;

use anyhow::anyhow;
use anyhow::Result;
use btleplug::api::bleuuid::uuid_from_u16;
use btleplug::api::Characteristic;
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

pub struct FitnessMachine {
    pub client: Peripheral,
    control_point: Characteristic,
    status: Characteristic,
    feature: Characteristic,
}

impl FitnessMachine {
    pub async fn new(ble: &BleClient) -> Result<FitnessMachine> {
        let res = ble.find_service(SERVICE_UUID).await?;

        if res.is_some() {
            let client = res.unwrap();

            let feature =
                get_characteristic(&client, FEATURE).ok_or(anyhow!("feature char not found!"))?;

            let control_point = get_characteristic(&client, CONTROL_POINT)
                .ok_or(anyhow!("control point char not found!"))?;

            let status =
                get_characteristic(&client, STATUS).ok_or(anyhow!("status char not found!"))?;

            Ok(FitnessMachine {
                client,
                control_point,
                status,
                feature,
            })
        } else {
            Err(anyhow!("Fitness machine device not found"))
        }
    }

    pub async fn dump_service_info(&self) -> Result<()> {
        let _: Vec<_> = self
            .client
            .services()
            .into_iter()
            .filter(|service| {
                if service.uuid == SERVICE_UUID {
                    info!("FITNESS MACHINE PROFILE");
                    true
                } else {
                    false
                }
            })
            .flat_map(|service| {
                info!("Characteristics:");
                service.characteristics.into_iter().map(|char| {
                    info!("    {:?}", char);
                })
            })
            .collect();

        Ok(())
    }

    pub(crate) async fn disconnect(&self) -> Result<()> {
        let name = self.client.properties().await?.unwrap().local_name.unwrap();
        info!("Disconnecting from {name}");
        self.client.disconnect().await?;

        Ok(())
    }

    pub async fn get_features(&self) -> Result<String>{

        loop {
            let raw = self.client.read(&self.feature).await?;
            info!("Raw {:?}", raw);

        }


        let resp = String::from_utf8(vec![])?;

        info!("Features: {resp}");

        Ok(resp)
    }
}

/// Helper function to find characteristic
fn get_characteristic(client: &Peripheral, char_uuid: Uuid) -> Option<Characteristic> {
    let mut found: Vec<_> = client
        .characteristics()
        .into_iter()
        .filter(|c| c.uuid == char_uuid)
        .collect();

    found.pop()
}
