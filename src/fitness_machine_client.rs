use std::pin::Pin;

use anyhow::anyhow;
use anyhow::Result;
use btleplug::api::bleuuid::uuid_from_u16;
use btleplug::api::Characteristic;
use btleplug::api::Peripheral as _;
use btleplug::api::ValueNotification;
use btleplug::platform::Peripheral;
use futures::Stream;
use futures::StreamExt;
use num_traits::FromPrimitive;

use byteorder::ByteOrder;
use byteorder::LittleEndian;
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::Sender;
use uuid::Uuid;

use crate::ble_client::BleClient;
use crate::scalar_converter::ScalarType;

// TODO: it's getting messy, refactor

/// GATTS Service UUID
const SERVICE_UUID: Uuid = uuid_from_u16(0x1826);

/// READ, Characteristic to retrieve supported features
/// Like cadence, power measurement, etc
const MACHINE_FEATURE: Uuid = uuid_from_u16(0x2ACC);

/// NOTIFY, gets current speed, cadence, power, etc
const INDOOR_BIKE_DATA: Uuid = uuid_from_u16(0x2AD2);

/// NOTIFY: something like, idle, warming up, low/high interval, fitness test, cool down, manual mode
const TRAINING_STATUS: Uuid = uuid_from_u16(0x2AD3);

/// READ: gets supported resistance level
const SUPPORTED_RESISTANCE_LEVEL: Uuid = uuid_from_u16(0x2AD6);

/// READ: gets supported power range
const SUPPORTED_POWER_RANGE: Uuid = uuid_from_u16(0x2AD8);

/// NOTIFY, gets machine status changes
const MACHINE_STATUS: Uuid = uuid_from_u16(0x2ADA);

/// INDICATE, WRITE send control messages
const CONTROL_POINT: Uuid = uuid_from_u16(0x2AD9);

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

/// Struct holding supported range of values to set for given characteristic
#[derive(Debug)]
struct Range<T, S = T> {
    min: T,
    max: T,
    step: S,
}

/// Implementation of FitnessMachine GATTS profile for Indoor Bike
pub struct IndoorBikeFitnessMachine {
    client: Peripheral,
    control_point: Characteristic,
    feature: Characteristic,
    resistance_range: Range<f64>,
    power_range: Range<i16, u16>,
    indoor_bike_tx: Sender<BikeData>,
    training_tx: Sender<String>,
    machine_tx: Sender<String>,
    control_point_tx: Sender<String>,
}

// TODO: this is very first implementation, that is not covering every possible indoor bike machine.
// Correct way of creation such object would be to read feature characteristic (which is mandatory to be present)
// and according to supported features add other characteristics, like control point, resistance level, power, etc.
impl IndoorBikeFitnessMachine {
    pub async fn new(ble: &BleClient) -> Result<IndoorBikeFitnessMachine> {
        info!("Creating Indoor Bike Fitness Machine...");
        let res = ble.find_service(SERVICE_UUID).await?;

        if res.is_some() {
            // Client representing the device that exposes fitness machine profile
            let client = res.unwrap();

            // Get characteristic from the profile
            let feature = get_characteristic(&client, MACHINE_FEATURE)
                .ok_or(anyhow!("feature char not found!"))?;

            let control_point = get_characteristic(&client, CONTROL_POINT)
                .ok_or(anyhow!("control point char not found!"))?;

            let (indoor_bike_tx, training_tx, machine_tx, control_point_tx) =
                subscribe_to_characteristics(&client).await?;

            let resistance_range = get_resistance_range(&client).await?;
            info!("Supported resistance range {resistance_range:?}");

            let power_range = get_power_range(&client).await?;
            info!("Supported power range {power_range:?}");

            Ok(IndoorBikeFitnessMachine {
                client,
                control_point,
                feature,
                resistance_range,
                power_range,
                indoor_bike_tx,
                training_tx,
                machine_tx,
                control_point_tx,
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
    pub fn subscribe_for_indoor_bike_notifications(&self) -> Receiver<BikeData> {
        let rx = self.indoor_bike_tx.subscribe();
        rx
    }

    pub fn subscribe_for_training_notifications(&self) -> Receiver<String> {
        let rx = self.training_tx.subscribe();
        rx
    }

    pub fn subscribe_for_machine_notifications(&self) -> Receiver<String> {
        let rx = self.machine_tx.subscribe();
        rx
    }

    pub fn subscribe_for_control_point_notifications(&self) -> Receiver<String> {
        let rx = self.control_point_tx.subscribe();
        rx
    }
}

/// Subscribe to all characteristics, and provide channels to access the data
async fn subscribe_to_characteristics(
    client: &Peripheral,
) -> Result<(
    Sender<BikeData>,
    Sender<String>,
    Sender<String>,
    Sender<String>,
)> {
    for characteristic_uuid in [
        INDOOR_BIKE_DATA,
        TRAINING_STATUS,
        MACHINE_STATUS,
        CONTROL_POINT,
    ] {
        // TODO: now any of these is a fatal error, maybe don't be that picky
        let characteristic = get_characteristic(&client, characteristic_uuid)
            .ok_or(anyhow!("{characteristic_uuid:? }char not found!"))?;
        // Enable listening on notification's
        client.subscribe(&characteristic).await?;
    }

    // Create a broadcast channel for notification characteristic.
    // subscribers will receive rx endpoint of that channel
    let (indoor_tx, _) = tokio::sync::broadcast::channel(16);
    let (training_tx, _) = tokio::sync::broadcast::channel(16);
    let (machine_tx, _) = tokio::sync::broadcast::channel(16);
    let (control_point_tx, _) = tokio::sync::broadcast::channel(16);

    // Create a stream for incoming notifications
    let notifications = client.notifications().await?;

    // Handle notifications on separate task
    // TODO: should we do something with the handle?
    let notifications_handle = tokio::spawn(handle_notifications(
        notifications,
        indoor_tx.clone(),
        training_tx.clone(),
        machine_tx.clone(),
        control_point_tx.clone(),
    ));
    Ok((indoor_tx, training_tx, machine_tx, control_point_tx))
}

/// Gets range of valid power setting, data format defined in GATT_Specification_Supplement_v5
async fn get_power_range(client: &Peripheral) -> Result<Range<i16, u16>> {
    let power = get_characteristic(&client, SUPPORTED_POWER_RANGE)
        .ok_or(anyhow!("supported power level char not found!"))?;

    let raw = client.read(&power).await?;

    if raw.len() != 6 {
        return Err(anyhow!(
            "Invalid data format in supported power level char!"
        ));
    }

    let min = LittleEndian::read_i16(&raw[0..2]);
    let max = LittleEndian::read_i16(&raw[2..4]);
    let step = LittleEndian::read_u16(&raw[4..6]);

    Ok(Range { min, max, step })
}

/// Reads supported resistance level
/// field description in GATT_Specification_Supplement
async fn get_resistance_range(client: &Peripheral) -> Result<Range<f64>> {
    let resistance = get_characteristic(&client, SUPPORTED_RESISTANCE_LEVEL)
        .ok_or(anyhow!("supported resistance level char not found!"))?;

    let raw = client.read(&resistance).await?;

    // TODO: docs claim there should be 3 u8's but that's not true :/
    let min = LittleEndian::read_i16(&raw[0..2]);
    let max = LittleEndian::read_i16(&raw[2..4]);
    // TODO: should be u16 probably
    let step = LittleEndian::read_i16(&raw[4..6]);

    if raw.len() != 6 {
        return Err(anyhow!(
            "Invalid data format in supported resistance level char!"
        ));
    }

    let conv = ScalarType::new().with_multiplier(1).with_dec_exp(1);
    Ok(Range {
        min: conv.to_scalar(min),
        max: conv.to_scalar(max),
        step: conv.to_scalar(step),
    })
}

async fn handle_notifications(
    mut notifications: Pin<Box<dyn Stream<Item = ValueNotification> + Send>>,
    indoor_tx: Sender<BikeData>,
    training_tx: Sender<String>,
    machine_tx: Sender<String>,
    control_point_tx: Sender<String>,
) {
    // TODO: when it returns none?
    while let Some(data) = notifications.next().await {
        match data.uuid {
            MACHINE_STATUS => {
                debug!("Got notification from MACHINE_STATUS: {:?}", data.value);
            }
            INDOOR_BIKE_DATA => {
                debug!("Got notification from INDOOR_BIKE_DATA: {:?}", data.value);
                let parsed_data = handle_bike_data_notification(&data.value);

                indoor_tx.send(parsed_data).unwrap();
            }
            TRAINING_STATUS => {
                debug!("Got notification from TRAINING_STATUS: {:?}", data.value);
            }
            CONTROL_POINT => {
                debug!("Got notification from CONTROL_POINT: {:?}", data.value);
            }
            _ => {
                warn!(
                    "Got unhandled notification from uuid {}, value {:?}",
                    data.uuid, data.value
                );
            }
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct BikeData {
    inst_speed: Option<f64>,
    avg_speed: Option<f64>,
    inst_cadence: Option<f64>,
    avg_cadence: Option<f64>,
    tot_distance: Option<u32>,
    resistance_lvl: Option<f64>,
    inst_power: Option<i16>,
    avg_power: Option<i16>,
    elapsed_time: Option<u16>,
    remaining_time: Option<u16>,
}

fn handle_bike_data_notification(value: &[u8]) -> BikeData {
    #[derive(Debug, FromPrimitive)]
    enum Flags {
        MoreData = 1 << 0, // a.k.a instantaneous speed, this is f*kd up
        AvgSpeed = 1 << 1,
        InstCadence = 1 << 2,
        AvgCadence = 1 << 3,
        TotDistance = 1 << 4,
        ResistanceLvl = 1 << 5,
        InstPower = 1 << 6,
        AvgPower = 1 << 7,
        ExpendedEnergy = 1 << 8,
        HR = 1 << 9,
        MetabolicEquivalent = 1 << 10,
        ElapsedTime = 1 << 11,
        RemainingTime = 1 << 12,
    }
    const FLAGS_LEN: u16 = 13;

    let flags = LittleEndian::read_u16(&value[0..]);

    // Start after flag field
    let mut cursor = 2;

    let mut data = BikeData::default();

    // For inst speed logic is reversed, additionally this field contains 2 different things
    // depending on value.
    if flags & Flags::MoreData as u16 == 1 {
        // If set to 1, means there will be more data to come
        // Happens when data does not fit into UTU
        unimplemented!("More Data scenario is not yet implemented")
    } else {
        // If set to zero, it actually means field represents instantaneous speed
        let raw = LittleEndian::read_u16(&value[cursor..]);
        cursor += 2;

        let conv = ScalarType::new().with_multiplier(1).with_dec_exp(-2);
        data.inst_speed = Some(conv.to_scalar(raw));
    }

    // Check flags bit, if set then there is a value in the data stream corresponding to that field
    for i in 1..FLAGS_LEN {
        let field_present: u16 = flags & (1 << i);

        match Flags::from_u16(field_present).unwrap() {
            Flags::AvgSpeed => {
                let raw = LittleEndian::read_u16(&value[cursor..]);
                cursor += 2;

                let conv = ScalarType::new().with_multiplier(1).with_dec_exp(-2);
                data.avg_speed = Some(conv.to_scalar(raw));
            }
            Flags::InstCadence => {
                let raw = LittleEndian::read_u16(&value[cursor..]);
                cursor += 2;

                let conv = ScalarType::new().with_multiplier(1).with_dec_exp(-1);
                data.inst_cadence = Some(conv.to_scalar(raw));
            }
            Flags::AvgCadence => {
                let raw = LittleEndian::read_u16(&value[cursor..]);
                cursor += 2;

                let conv = ScalarType::new().with_multiplier(1).with_dec_exp(-1);
                data.avg_cadence = Some(conv.to_scalar(raw));
            }
            Flags::TotDistance => {
                let raw = LittleEndian::read_u24(&value[cursor..]);
                cursor += 3;

                data.tot_distance = Some(raw);
            }
            Flags::ResistanceLvl => {
                let raw = value[cursor];
                cursor += 1;

                let conv = ScalarType::new().with_multiplier(1).with_dec_exp(1);
                data.resistance_lvl = Some(conv.to_scalar(raw));
            }
            Flags::InstPower => {
                let raw = LittleEndian::read_i16(&value[cursor..]);
                cursor += 2;

                data.inst_power = Some(raw);
            }
            Flags::AvgPower => {
                let raw = LittleEndian::read_i16(&value[cursor..]);
                cursor += 2;

                data.avg_power = Some(raw);
            }
            Flags::ElapsedTime => {
                let raw = LittleEndian::read_u16(&value[cursor..]);
                cursor += 2;

                data.elapsed_time = Some(raw);
            }
            Flags::RemainingTime => {
                let raw = LittleEndian::read_u16(&value[cursor..]);
                cursor += 2;

                data.remaining_time = Some(raw);
            }
            Flags::MoreData => unreachable!(),
            Flags::MetabolicEquivalent => {
                unimplemented!("parsing MetabolicEquivalent data not implemented")
            }
            Flags::HR => unimplemented!("parsing HR data not implemented"),
            Flags::ExpendedEnergy => unimplemented!("parsing ExpendedEnergy data not implemented"),
        };
    }

    debug!("Parsed bike data {data:?}");
    data
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
