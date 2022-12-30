use std::{collections::VecDeque, path::Path, task::Poll, time::Duration};

use anyhow::{Context, Result};
use futures::{Future, Stream};

use serde::{Deserialize, Serialize};
use serde_xml_rs::from_str;
use tokio::{
    io::AsyncReadExt,
    pin,
    task::JoinHandle,
    time::{self},
};

use crate::{
    cli::UserCommands,
    zwo_workout_steps::{PowerDuration, WorkoutSteps},
};

pub struct ZwoWorkout {
    workout: WorkoutFile,
    pending: Option<JoinHandle<()>>,
    current_step: WorkoutSteps,
    ftp_base: f64,
}

// XML schema definition
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase")]
struct WorkoutFile {
    author: String,
    name: String,
    description: String,
    sport_type: String,
    workout: Workout,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Workout {
    #[serde(rename = "$value")]
    workouts: VecDeque<WorkoutSteps>,
}

impl ZwoWorkout {
    pub(crate) async fn new(workout_path: &Path, ftp_base: f64) -> Result<Self> {
        let mut file = tokio::fs::File::open(workout_path).await?;

        let mut content = String::new();
        let _read = file
            .read_to_string(&mut content)
            .await
            .context("Reading xml to String failed")?;

        let mut workout: WorkoutFile =
            from_str(&content).context("Parsing xml string to Workouts struct failed")?;
        trace!("Parsed xml {workout:#?}");

        info!("Loaded {}", workout_path.display());

        let current_step = workout
            .workout
            .workouts
            .pop_front()
            .expect("Workout does not contain any workout steps");

        info!("Next step {current_step:?}");

        Ok(ZwoWorkout {
            workout,
            pending: None,
            current_step,
            ftp_base,
        })
    }

    fn advance_workout(&mut self) -> Option<PowerDuration> {
        if let Some(next_step) = self.current_step.advance() {
            return Some(next_step);
        }

        // Current step exhausted, get next one
        let next = self.workout.workout.workouts.pop_front();

        info!("Next step {next:?}");

        // Nothing left
        if next.is_none() {
            // TODO: get rid off Poll enum from here
            return None;
        }

        // Start with next workout
        self.current_step = next.unwrap();

        let next_step = self
            .current_step
            .advance()
            .expect("Cannot advance fresh workout step");

        return Some(next_step);
    }

    fn get_power(&self, power_level: f64) -> i16 {
        (self.ftp_base * power_level).round() as i16
    }
}

// Stream trait cannot have private helper methods...
// Add another trait to separate stream-like logic from
// workout logic
trait StreamHelper {
    fn setup_timer(&mut self, duration: Duration, cx: &mut std::task::Context<'_>);
}

impl StreamHelper for ZwoWorkout {
    fn setup_timer(&mut self, duration: Duration, cx: &mut std::task::Context<'_>) {
        // Wake the stream when timer fires up - then return next workout
        let waker = cx.waker().clone();
        // TODO: there should be a way to use time::interval().poll_tick
        let handle = tokio::spawn(async move {
            time::sleep(duration).await;
            waker.wake();
        });

        self.pending = Some(handle);
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
                // Timer already fired before poll_next was called on the iterator.
                // In such case return next workout immediately
                Poll::Ready(_) => {
                    warn!("Returning stale workout data!");
                    // TODO: repetition of the code below
                    match self.advance_workout() {
                        Some(PowerDuration {
                            duration,
                            power_level,
                        }) => {
                            self.setup_timer(duration, cx);

                            return Poll::Ready(Some(UserCommands::SetTargetPower {
                                power: self.get_power(power_level),
                            }));
                        }

                        // Whole workout exhausted
                        None => return Poll::Ready(None),
                    }
                }
                // Previous workout should be still executed
                Poll::Pending => return Poll::Pending,
            }
        } else {
            // No workout pending, get next one
            match self.advance_workout() {
                Some(PowerDuration {
                    duration,
                    power_level,
                }) => {
                    self.setup_timer(duration, cx);

                    return Poll::Ready(Some(UserCommands::SetTargetPower {
                        power: self.get_power(power_level),
                    }));
                }

                // Whole workout exhausted
                None => return Poll::Ready(None),
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.workout.workout.workouts.len()))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use walkdir::WalkDir;

    use super::*;

    #[tokio::test]
    async fn can_correctly_parse_all_workouts() {
        let workouts_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("workouts");

        for entry in WalkDir::new(workouts_root)
            .into_iter()
            .filter_map(|e| match e {
                Ok(entry) => {
                    if entry.file_type().is_file() {
                        Some(entry)
                    } else {
                        None
                    }
                }
                Err(_) => None,
            })
        {
            println!("{}", entry.path().display());
            ZwoWorkout::new(entry.path(), 100.0).await.unwrap();
        }
    }
}
