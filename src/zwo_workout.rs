use std::{collections::VecDeque, path::Path, task::Poll, time::Duration};

use anyhow::{Context, Result};
use futures::{
    future::{poll_fn, Pending},
    Future, FutureExt, Stream,
};

use serde::{Deserialize, Serialize};
use serde_xml_rs::from_str;
use tokio::{
    io::AsyncReadExt,
    pin,
    time::{self, Instant, Interval, interval}, task::JoinHandle,
};

use crate::cli::UserCommands;

pub struct ZwoWorkout {
    workout: workout_file,
    pending: Option<JoinHandle<()>>,
}

// XML schema definition
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[allow(non_snake_case, non_camel_case_types)]
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
    workouts: VecDeque<WorkoutTypes>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum WorkoutTypes {
    Warmup(Warmup),
    SteadyState(SteadyState),
    Cooldown(Cooldown),
    IntervalsT(IntervalsT),
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[allow(non_snake_case)]
struct Warmup {
    Duration: usize,
    PowerLow: f64,
    PowerHigh: f64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[allow(non_snake_case)]
struct Cooldown {
    Duration: usize,
    PowerLow: f64,
    PowerHigh: f64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[allow(non_snake_case)]
struct SteadyState {
    Duration: usize,
    Power: f64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[allow(non_snake_case)]
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

        trace!("Parsed xml {workout:#?}");

        Ok(ZwoWorkout {
            workout,
            pending: None,
        })
    }
}

impl Stream for ZwoWorkout {
    type Item = UserCommands;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {

        if let Some(handle) = self.pending.take() {
            pin!(handle);

            match handle.poll(cx) {
                // Timer fired before poll_next was called on the iterator.
                // In such case return next workout immediately
                Poll::Ready(_) => {
                    debug!("Timer fired!");
                    let next_workout = match self.as_mut().workout.workout.workouts.pop_front() {
                        Some(workout) => {
                            debug!("Workout in the stream {workout:?}");

                            let interval = Duration::from_millis(100);

                            let waker = cx.waker().clone();
                            let handle = tokio::spawn(async move {
                                let mut interval = time::interval(interval);
                                interval.tick().await;
                                interval.tick().await;
                                waker.wake();
                            });

                            self.pending = Some(handle);

                            // First tick fires up immediately, starting from this instant next interval is waited
                            // info!("new {:?}", self.pending.poll_tick(cx));

                            Poll::Ready(Some(UserCommands::SetTargetPower { power: 100 }))
                        }
                        None => Poll::Ready(None),
                    };

                    debug!("debug: return ready");
                    return next_workout;
                },
                // Previous workout should be still executed
                Poll::Pending => return Poll::Pending,
            }

        } else {
            debug!("Timer fired!");
            let next_workout = match self.as_mut().workout.workout.workouts.pop_front() {
                Some(workout) => {
                    debug!("Workout in the stream {workout:?}");

                    let interval = Duration::from_millis(100);

                    let waker = cx.waker().clone();
                    let handle = tokio::spawn(async move {
                        time::sleep(interval).await;
                        waker.wake();
                    });

                    self.pending = Some(handle);

                    Poll::Ready(Some(UserCommands::SetTargetPower { power: 100 }))
                }
                None => Poll::Ready(None),
            };

            debug!("debug: return ready");
            return next_workout;
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.workout.workout.workouts.len()))
    }
}
