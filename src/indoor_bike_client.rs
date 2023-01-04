//! Implementation of GATTS Fitness Machine of type Indoor Bike
//! Refer to BLE GATTS Fitness Machine Profile documentation
use std::pin::Pin;

use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;

use btleplug::api::Characteristic;
use btleplug::api::Peripheral as _;
use btleplug::api::ValueNotification;
use btleplug::api::WriteType;
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
use crate::indoor_bike_data_defs::BikeData;
use crate::indoor_bike_data_defs::BikeDataFlags;
use crate::indoor_bike_data_defs::ControlPointNotificationData;
use crate::indoor_bike_data_defs::ControlPointOpCode;
use crate::indoor_bike_data_defs::ControlPointResult;
use crate::indoor_bike_data_defs::FitnessMachineFeatures;
use crate::indoor_bike_data_defs::MachineStatusOpCode;
use crate::indoor_bike_data_defs::Range;
use crate::indoor_bike_data_defs::TargetSettingFeatures;
use crate::indoor_bike_data_defs::BIKE_DATA_FLAGS_LEN;
use crate::indoor_bike_data_defs::CONTROL_POINT;
use crate::indoor_bike_data_defs::FITNESS_MACHINE_FEATURES_LEN;
use crate::indoor_bike_data_defs::INDOOR_BIKE_DATA;
use crate::indoor_bike_data_defs::MACHINE_FEATURE;
use crate::indoor_bike_data_defs::MACHINE_STATUS;
use crate::indoor_bike_data_defs::SERVICE_UUID;
use crate::indoor_bike_data_defs::SUPPORTED_POWER_RANGE;
use crate::indoor_bike_data_defs::SUPPORTED_RESISTANCE_LEVEL;
use crate::indoor_bike_data_defs::TARGET_SETTING_FEATURES_LEN;
use crate::indoor_bike_data_defs::TRAINING_STATUS;
use crate::scalar_converter::ScalarType;

// TODO: it's getting messy, refactor

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
    control_point_tx: Sender<ControlPointNotificationData>,
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
                .ok_or_else(|| anyhow!("feature char not found!"))?;

            let control_point = get_characteristic(&client, CONTROL_POINT)
                .ok_or_else(|| anyhow!("control point char not found!"))?;

            let (indoor_bike_tx, training_tx, machine_tx, control_point_tx) =
                subscribe_to_characteristics(&client).await?;

            let resistance_range = get_resistance_range(&client).await?;
            info!("Supported resistance range {resistance_range:?}");

            let power_range = get_power_range(&client).await?;
            info!("Supported power range {power_range:?}");

            let indoor_bike = IndoorBikeFitnessMachine {
                client,
                control_point,
                feature,
                resistance_range,
                power_range,
                indoor_bike_tx,
                training_tx,
                machine_tx,
                control_point_tx,
            };

            // TODO: we should wait for control point indication that this operation succeeded
            // before doing any other writes
            indoor_bike.request_control().await?;

            Ok(indoor_bike)
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

        trace!("Feature raw response {raw:?}");
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
        
        self.indoor_bike_tx.subscribe()
    }

    pub fn subscribe_for_training_notifications(&self) -> Receiver<String> {
        
        self.training_tx.subscribe()
    }

    pub fn subscribe_for_machine_notifications(&self) -> Receiver<String> {
        
        self.machine_tx.subscribe()
    }

    pub fn subscribe_for_control_point_notifications(&self) -> Receiver<ControlPointNotificationData> {
        
        self.control_point_tx.subscribe()
    }

    pub async fn set_resistance(&self, _resistance: u8) -> Result<()> {
        // if !self.resistance_range.in_range(resistance) {
        //     return Err(anyhow!("Resistance {resistance} outside valid range {:?}", self.resistance_range));
        // }
        // let data: [u8; 1] = [ControlPoint::RequestControl as u8];
        // self.client
        //     .write(&self.control_point, &data, WriteType::WithResponse)
        //     .await?;

        // let data : [u8; 2] = [ControlPoint::SetTargetResistance as u8, resistance];

        // self.client
        //     .write(&self.control_point, &data, WriteType::WithResponse)
        //     .await?;

        // Ok(())

        todo!()
    }

    pub async fn set_power(&self, power: i16) -> Result<()> {
        if !self.power_range.in_range(power) {
            return Err(anyhow!(
                "Resistance {power} outside valid range {:?}",
                self.power_range
            ));
        }

        let mut data: [u8; 3] = [ControlPointOpCode::SetTargetPower as u8, 0, 0];

        LittleEndian::write_i16(&mut data[1..], power);

        match self
            .client
            .write(&self.control_point, &data, WriteType::WithResponse)
            .await
            .context("while setting power")
        {
            Ok(_) => debug!("Set power succeeded"),
            Err(e) => error!("Failed to set power: '{e:?}', continuing"),
        }

        Ok(())
    }

    /// The control permission remains valid until the connection is terminated, the notification of the Fitness
    /// Machine Status is sent with the value set to Control Permission Lost
    async fn request_control(&self) -> Result<()> {
        let data: [u8; 1] = [ControlPointOpCode::RequestControl as u8];
        self.client
            .write(&self.control_point, &data, WriteType::WithResponse)
            .await
            .context("while sending request control")?;

        Ok(())
    }
}

/// Subscribe to all characteristics, and provide channels to access the data
async fn subscribe_to_characteristics(
    client: &Peripheral,
) -> Result<(
    Sender<BikeData>,
    Sender<String>,
    Sender<String>,
    Sender<ControlPointNotificationData>,
)> {
    for characteristic_uuid in [
        INDOOR_BIKE_DATA,
        TRAINING_STATUS,
        MACHINE_STATUS,
        CONTROL_POINT,
    ] {
        // TODO: now any of these is a fatal error, maybe don't be that picky
        let characteristic = get_characteristic(client, characteristic_uuid)
            .ok_or_else(|| anyhow!("{characteristic_uuid:? }char not found!"))?;
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
    let _notifications_handle = tokio::spawn(handle_notifications(
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
    let power = get_characteristic(client, SUPPORTED_POWER_RANGE)
        .ok_or_else(|| anyhow!("supported power level char not found!"))?;

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
    let resistance = get_characteristic(client, SUPPORTED_RESISTANCE_LEVEL)
        .ok_or_else(|| anyhow!("supported resistance level char not found!"))?;

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
    _training_tx: Sender<String>,
    _machine_tx: Sender<String>,
    control_point_tx: Sender<ControlPointNotificationData>,
) {
    // TODO: when it returns none?
    while let Some(data) = notifications.next().await {
        match data.uuid {
            MACHINE_STATUS => {
                trace!("Got notification from MACHINE_STATUS: {:?}", data.value);
                handle_machine_status_notification(&data.value);

                // TODO:
                // let _ = machine_tx.send(parsed_data);
            }
            INDOOR_BIKE_DATA => {
                trace!("Got notification from INDOOR_BIKE_DATA: {:?}", data.value);
                let parsed_data = handle_bike_data_notification(&data.value);

                // Send may fail, if there is no receiver
                let _ = indoor_tx.send(parsed_data);
            }
            TRAINING_STATUS => {
                trace!("Got notification from TRAINING_STATUS: {:?}", data.value);
            }
            CONTROL_POINT => {
                trace!("Got notification from CONTROL_POINT: {:?}", data.value);
                let cp_response = handle_control_point_notification(&data.value);
                let _ = control_point_tx.send(cp_response);
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

fn handle_control_point_notification(raw_data: &[u8]) -> ControlPointNotificationData {
    let op_code = raw_data[0];
    assert_eq!(op_code, 0x80);

    let request_response = ControlPointNotificationData {
        request_op_code: ControlPointOpCode::from_u8(raw_data[1]).unwrap(),
        request_status: ControlPointResult::from_u8(raw_data[2]).unwrap(),
    };

    debug!("Control Point Notification response {request_response:?}");

    request_response
}

fn handle_machine_status_notification(raw_data: &[u8]) {
    let op_code = raw_data[0];

    let parsed_op_code = MachineStatusOpCode::from_u8(op_code).unwrap();
    debug!("Got Machine Status Notification with opcode {parsed_op_code:?}");
}

/// Handle raw stream from notification into BikeData
fn handle_bike_data_notification(raw_data: &[u8]) -> BikeData {
    let flags = LittleEndian::read_u16(&raw_data[0..]);

    // Cursor pointing current position in raw_data
    // Start after flag field
    let mut cursor = 2;

    let mut bike_data = BikeData::default();

    // For inst speed logic is reversed, additionally this field contains 2 different things
    // depending on value.
    if flags & BikeDataFlags::MoreData as u16 == 1 {
        // If set to 1, means there will be more data to come
        // Happens when data does not fit into UTU
        unimplemented!("More Data scenario is not yet implemented")
    } else {
        // If set to zero, it actually means field represents instantaneous speed
        let raw = LittleEndian::read_u16(&raw_data[cursor..]);
        // jump to another field
        cursor += 2;

        let conv = ScalarType::new().with_multiplier(1).with_dec_exp(-2);
        bike_data.inst_speed = Some(conv.to_scalar(raw));
    }

    // Check flags bit, if set then there is a value in the data stream corresponding to that field
    for i in 1..BIKE_DATA_FLAGS_LEN {
        let field_present: u16 = flags & (1 << i);

        if field_present == 0 {
            // Given field not present
            continue;
        }

        match BikeDataFlags::from_u16(field_present).unwrap() {
            BikeDataFlags::AvgSpeed => {
                let raw = LittleEndian::read_u16(&raw_data[cursor..]);
                cursor += 2;

                let conv = ScalarType::new().with_multiplier(1).with_dec_exp(-2);
                bike_data.avg_speed = Some(conv.to_scalar(raw));
            }
            BikeDataFlags::InstCadence => {
                let raw = LittleEndian::read_u16(&raw_data[cursor..]);
                cursor += 2;

                let conv = ScalarType::new().with_multiplier(1).with_dec_exp(-1);
                bike_data.inst_cadence = Some(conv.to_scalar(raw));
            }
            BikeDataFlags::AvgCadence => {
                let raw = LittleEndian::read_u16(&raw_data[cursor..]);
                cursor += 2;

                let conv = ScalarType::new().with_multiplier(1).with_dec_exp(-1);
                bike_data.avg_cadence = Some(conv.to_scalar(raw));
            }
            BikeDataFlags::TotDistance => {
                let raw = LittleEndian::read_u24(&raw_data[cursor..]);
                cursor += 3;

                bike_data.tot_distance = Some(raw);
            }
            BikeDataFlags::ResistanceLvl => {
                let raw = raw_data[cursor];
                cursor += 1;

                let conv = ScalarType::new().with_multiplier(1).with_dec_exp(1);
                bike_data.resistance_lvl = Some(conv.to_scalar(raw));
            }
            BikeDataFlags::InstPower => {
                let raw = LittleEndian::read_i16(&raw_data[cursor..]);
                cursor += 2;

                bike_data.inst_power = Some(raw);
            }
            BikeDataFlags::AvgPower => {
                let raw = LittleEndian::read_i16(&raw_data[cursor..]);
                cursor += 2;

                bike_data.avg_power = Some(raw);
            }
            BikeDataFlags::ElapsedTime => {
                let raw = LittleEndian::read_u16(&raw_data[cursor..]);
                cursor += 2;

                bike_data.elapsed_time = Some(raw);
            }
            BikeDataFlags::RemainingTime => {
                let raw = LittleEndian::read_u16(&raw_data[cursor..]);
                cursor += 2;

                bike_data.remaining_time = Some(raw);
            }
            BikeDataFlags::MoreData => unreachable!(),
            BikeDataFlags::MetabolicEquivalent => {
                unimplemented!("parsing MetabolicEquivalent data not implemented")
            }
            BikeDataFlags::HR => unimplemented!("parsing HR data not implemented"),
            BikeDataFlags::ExpendedEnergy => {
                unimplemented!("parsing ExpendedEnergy data not implemented")
            }
        };
    }

    trace!("Parsed bike data {bike_data:#?}");
    bike_data
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
