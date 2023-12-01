//! Data format definitions for Indoor Bike

// Endpoints, aka Characteristics

use btleplug::api::bleuuid::uuid_from_u16;
use uuid::Uuid;

/// GATTS Service UUID
pub const SERVICE_UUID: Uuid = uuid_from_u16(0x1826);

/// READ, Characteristic to retrieve supported features
/// Like cadence, power measurement, etc
pub const MACHINE_FEATURE: Uuid = uuid_from_u16(0x2ACC);

/// NOTIFY, gets current speed, cadence, power, etc
pub const INDOOR_BIKE_DATA: Uuid = uuid_from_u16(0x2AD2);

/// NOTIFY: something like, idle, warming up, low/high interval, fitness test, cool down, manual mode
pub const TRAINING_STATUS: Uuid = uuid_from_u16(0x2AD3);

/// READ: gets supported resistance level
pub const SUPPORTED_RESISTANCE_LEVEL: Uuid = uuid_from_u16(0x2AD6);

/// READ: gets supported power range
pub const SUPPORTED_POWER_RANGE: Uuid = uuid_from_u16(0x2AD8);

/// NOTIFY, gets machine status changes
pub const MACHINE_STATUS: Uuid = uuid_from_u16(0x2ADA);

/// INDICATE, WRITE send control messages
pub const CONTROL_POINT: Uuid = uuid_from_u16(0x2AD9);

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
pub const FITNESS_MACHINE_FEATURES_LEN: u32 = 17;

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
pub const TARGET_SETTING_FEATURES_LEN: u32 = 17;

/// Representation of data from Indoor Bike Data characteristic
///  BikeData has different fields present, depending on flag field
#[derive(Debug, Default, Clone)]
pub struct BikeData {
    pub inst_speed: Option<f64>,
    pub avg_speed: Option<f64>,
    pub inst_cadence: Option<f64>,
    pub avg_cadence: Option<f64>,
    pub tot_distance: Option<u32>,
    pub resistance_lvl: Option<f64>,
    pub inst_power: Option<i16>,
    pub avg_power: Option<i16>,
    pub elapsed_time: Option<u16>,
    pub remaining_time: Option<u16>,
}

#[derive(Debug, FromPrimitive)]
pub enum BikeDataFlags {
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
pub const BIKE_DATA_FLAGS_LEN: u16 = 13;

/// Machine indicates about it's internal state change
#[derive(Debug, FromPrimitive)]
pub enum MachineStatusOpCode {
    Reserved0 = 0x0,
    Reset = 0x1,
    StoppedPausedByUser = 0x2,
    StoppedBySafetyKey = 0x3,
    StartedResumedByUser = 0x4,
    TargetSpeedChanged = 0x5,
    TargetInclineChanged = 0x6,
    TargetResistanceChanged = 0x7,
    TargetPowerChanged = 0x8,
    TargetHRChanged = 0x9,
    TargetedExpendedEnergyChanged = 0xA,
    TargetedNumberOfStepsChanged = 0xB,
    TargetedNumberOfStridesChanged = 0xC,
    TargetedDistanceChanged = 0xD,
    TargetedTrainingTimeChanged = 0xE,
    TargetedTimeIn2HRZonesChanged = 0xF,
    TargetedTimeIn3HRZonesChanged = 0x10,
    TargetedTimeIn5HRZonesChanged = 0x11,
    IndoorBikeSimulationParametersChanged = 0x12,
    WheelCircumferenceChanged = 0x13,
    SpinDownStatus = 0x14,
    // 0x15-0xfe reserved
    ControlPermissionLost = 0xFF,
}

// TODO: added only those supported by SUITO
/// Thing you can change using control point, followed by parameter
/// DOCS: FTMS_v1.0 4.16.1, Table 4.15
#[derive(Debug, FromPrimitive, Clone)]
pub enum ControlPointOpCode {
    RequestControl = 0x0,
    // Set machine fields to default, like elapsed time to 0, etc. sets training status to idle
    Reset = 0x1,
    SetTargetResistance = 0x4,
    SetTargetPower = 0x5,
    StartOrResume = 0x7,
    StopOrPause = 0x8,
    IndoorBikeSimulation = 0x11,
    WheelCircumference = 0x12,
    SpinDownControl = 0x13,
}

/// Control Point sends an indication as a response to the write request, with given status
/// DOCS: FTMS_v1.0 4.16.1 Table 4.24
#[derive(Debug, FromPrimitive, Clone)]
pub enum ControlPointResult {
    Reserved0 = 0x0,
    Success = 0x1,
    OpCodeNotSupported = 0x2,
    InvalidParam = 0x3,
    OperationFailed = 0x4,
    ControlNotPermitted = 0x5,
    // 0x06-0xff - reserved
}

/// Data that is returned by control point indication
/// It's a response to write request that happened prior that
#[derive(Debug, Clone)]
pub struct ControlPointNotificationData {
    pub request_op_code: ControlPointOpCode,
    pub request_status: ControlPointResult,
}
/// Struct holding supported range of values to set for given characteristic
#[derive(Debug)]
pub struct Range<T, S = T> {
    pub min: T,
    pub max: T,
    pub step: S,
}
impl<T, S> Range<T, S>
where
    T: PartialOrd,
{
    pub(crate) fn in_range(&self, value: T) -> bool {
        value >= self.min && value <= self.max
    }
}
