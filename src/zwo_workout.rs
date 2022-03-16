use std::path::Path;

use anyhow::{Context, Result};
use futures::Stream;

use serde::{Deserialize, Serialize};
use serde_xml_rs::{from_str, to_string};
use tokio::io::AsyncReadExt;

use crate::cli::UserCommands;

pub struct ZwoWorkout {
    workout: workout_file,
}

// XML schema definition
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct workout_file {
    author: String,
    name: String,
    description: String,
    sportType: String,
    workout: Workout,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Workout {
    #[serde(rename = "$value")]
    workouts: Vec<WorkoutTypes>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum WorkoutTypes {
    Warmup(Warmup),
    SteadyState(SteadyState),
    Cooldown(Cooldown),
    IntervalsT(IntervalsT),
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Warmup {
    Duration: usize,
    PowerLow: f64,
    PowerHigh: f64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Cooldown {
    Duration: usize,
    PowerLow: f64,
    PowerHigh: f64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct SteadyState {
    Duration: usize,
    Power: f64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct IntervalsT {
    Repeat: usize,
    OnDuration: usize,
    OffDuration: usize,
    OnPower: f64,
    OffPower: f64,
}

impl ZwoWorkout {
    pub(crate) async fn new(workout: &Path) -> Result<Self> {
        let mut file = tokio::fs::File::open(workout).await?;

        let mut content = String::new();
        let _read = file
            .read_to_string(&mut content)
            .await
            .context("Reading xml to String failed")?;

        let workout = from_str(&content).context("Parsing xml string to Workouts struct failed")?;

        info!("Parsed xml {workout:#?}");

        Ok(ZwoWorkout { workout })
    }
}

impl Stream for ZwoWorkout {
    type Item = UserCommands;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        todo!()
    }
}

#[test]
fn parse_xml_test() {
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct PlateAppearance {
        #[serde(rename = "$value")]
        events: Vec<Event>,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "kebab-case")]
    enum Event {
        Pitch(Pitch),
        Runner(Runner),
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Pitch {
        speed: u32,
        r#type: PitchType,
        outcome: PitchOutcome,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    enum PitchType {
        FourSeam,
        TwoSeam,
        Changeup,
        Cutter,
        Curve,
        Slider,
        Knuckle,
        Pitchout,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    enum PitchOutcome {
        Ball,
        Strike,
        Hit,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Runner {
        from: Base,
        to: Option<Base>,
        outcome: RunnerOutcome,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    enum Base {
        First,
        Second,
        Third,
        Home,
    }
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    enum RunnerOutcome {
        Steal,
        Caught,
        PickOff,
    }

    let document = r#"
        <plate-appearance>
          <pitch speed="95" type="FourSeam" outcome="Ball" />
          <pitch speed="91" type="FourSeam" outcome="Strike" />
          <pitch speed="85" type="Changeup" outcome="Ball" />
          <runner from="First" to="Second" outcome="Steal" />
          <pitch speed="89" type="Slider" outcome="Strike" />
          <pitch speed="88" type="Curve" outcome="Hit" />
        </plate-appearance>"#;
    let plate_appearance: PlateAppearance = from_str(document).unwrap();

    println!("PArsed {plate_appearance:?}");
}
