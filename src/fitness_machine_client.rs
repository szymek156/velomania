use anyhow::anyhow;
use anyhow::Result;
use btleplug::api::bleuuid::uuid_from_u16;
use btleplug::api::Characteristic;
use btleplug::api::Peripheral as _;
use btleplug::platform::Peripheral;
use futures::channel::mpsc::Sender;
use futures::StreamExt;
use num_traits::FromPrimitive;

use byteorder::ByteOrder;
use byteorder::LittleEndian;
use tokio::sync::broadcast::Receiver;
use tokio::task::JoinHandle;
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

#[derive(Debug, FromPrimitive)]
#[non_exhaustive]
pub enum FitnessMachineFeatures {
    AvgSpeed = 1 << 0,
    Cadence = 1 << 1,
    TotalDistance = 1 << 2,
    Inclination = 1 << 3,
    Elevation = 1 << 4,
    Pace = 1 << 5,
    StepCount = 1 << 6,
    Resistance = 1 << 7,
    StrideCount = 1 << 8,
    ExpendedEnergy = 1 << 9,
    HRMeasurement = 1 << 10,
    MetabolicEquivalent = 1 << 11,
    ElapsedTime = 1 << 12,
    RemainingTime = 1 << 13,
    PowerMeasurement = 1 << 14,
    ForceOnBeltAndPowerOutputSupported = 1 << 15,
    UserDataRetention = 1 << 16,
}
const FITNESS_MACHINE_FEATURES_LEN: u32 = 17;

#[derive(Debug, FromPrimitive)]
#[non_exhaustive]
pub enum TargetSettingFeatures {
    SpeedTarget = 1 << 0,
    Inclination = 1 << 1,
    Resistance = 1 << 2,
    Power = 1 << 3,
    HR = 1 << 4,
    TargetedExpendedEnergyConfiguration = 1 << 5,
    TargetedStepNumber = 1 << 6,
    TargetedStrideNumber = 1 << 7,
    TargetedDistance = 1 << 8,
    TargetedTrainingTime = 1 << 9,
    TargetedTimeIn2HRZones = 1 << 10,
    TargetedTimeIn3HRZones = 1 << 11,
    TargetedTimeIn5HRZones = 1 << 12,
    IndoorBikeSimulation = 1 << 13,
    WheelCircumference = 1 << 14,
    SpinDownControl = 1 << 15,
    TargetedCadence = 1 << 16,
}
const TARGET_SETTING_FEATURES_LEN: u32 = 17;
pub struct FitnessMachine {
    pub client: Peripheral,
    control_point: Characteristic,
    status: Characteristic,
    feature: Characteristic,
    notifications_handle: JoinHandle<()>,
    status_notifications_channel: tokio::sync::broadcast::Sender<String>,
}

impl FitnessMachine {
    pub async fn new(ble: &BleClient) -> Result<FitnessMachine> {
        let res = ble.find_service(SERVICE_UUID).await?;

        if res.is_some() {
            // Client representing the device that exposes fitness machine profile
            let client = res.unwrap();

            // Get characteristic from the profile
            let feature =
                get_characteristic(&client, FEATURE).ok_or(anyhow!("feature char not found!"))?;

            let control_point = get_characteristic(&client, CONTROL_POINT)
                .ok_or(anyhow!("control point char not found!"))?;

            let status =
                get_characteristic(&client, STATUS).ok_or(anyhow!("status char not found!"))?;

            // Enable listening on notifications
            client.subscribe(&status).await?;

            // Create a broadcast channel for status notification characteristic.
            // subscribers will receive rx endpoint of that channel
            let (tx, _) = tokio::sync::broadcast::channel(16);

            let tx1 = tx.clone();
            // Create a stream for incoming notifications
            let mut notifications = client.notifications().await?;

            // Handle notifications on separate task
            let notifications_handle = tokio::spawn(async move {
                // TODO: when it returns none?
                while let Some(data) = notifications.next().await {
                    debug!("Got notification with uuid {:?}", data.uuid);
                    if data.uuid == STATUS {
                        tx1.send("blablabla".to_string()).unwrap();
                    }
                }
            });

            Ok(FitnessMachine {
                client,
                control_point,
                status,
                feature,
                notifications_handle,
                status_notifications_channel: tx,
            })
        } else {
            Err(anyhow!("Fitness machine device not found"))
        }
    }

    /// Enumerate accessible characteristics for Fitness profile
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

    /// Get supported features for machine
    pub async fn get_features(&self) -> Result<()> {
        let raw = self.client.read(&self.feature).await?;

        if raw.len() != 8 {
            return Err(anyhow!(
                "Invalid data received from feature characteristic {raw:?}"
            ));
        }

        debug!("Feature raw response {raw:?}");
        let fitness_features = LittleEndian::read_u32(&raw[0..4]);

        info!("Fitness features supported:");
        for i in 0..FITNESS_MACHINE_FEATURES_LEN {
            let feature = 1 << i;
            if feature & fitness_features != 0 {
                info!(" {:?}", FitnessMachineFeatures::from_u32(feature));
            }
        }

        let target_setting_features = LittleEndian::read_u32(&raw[4..]);

        info!("Target setting features supported:");
        for i in 0..TARGET_SETTING_FEATURES_LEN {
            let feature = 1 << i;
            if feature & target_setting_features != 0 {
                info!("  {:?}", TargetSettingFeatures::from_u32(feature));
            }
        }

        // TODO: return struct?
        Ok(())
    }

    /// Get rx endpoint for status notifications
    /// To unsub, simply drop rx
    // TODO: let it be a string for now?
    pub fn subscribe_for_status_notifications(&self) -> Receiver<String> {
        let rx = self.status_notifications_channel.subscribe();
        rx
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
