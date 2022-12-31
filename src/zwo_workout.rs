use std::{collections::VecDeque, path::Path, task::Poll, time::Duration};

use anyhow::{Context, Result};
use futures::{Future, Stream};

use serde::{Deserialize, Serialize};

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
    workout_file: WorkoutFile,
    pending: Option<JoinHandle<()>>,
    pub workout_state: WorkoutState,
    // current_step: WorkoutSteps,
    // ftp_base: f64,
}

#[derive(Debug)]
pub struct WorkoutState {
    total_steps: usize,

    total_workout_duration: Duration,
    current_step_duration: Duration,

    current_step_idx: usize,
    current_step: WorkoutSteps,
    next_step: Option<WorkoutSteps>,

    current_power_set: i16,
    ftp_base: f64,
}
// XML schema definition
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
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

        let mut workout: WorkoutFile = serde_xml_rs::from_str(&content)
            .context("Parsing xml string to Workouts struct failed")?;
        trace!("Parsed xml {workout:#?}");

        info!("Loaded {}", workout_path.display());

        let total_workout_duration = calculate_total_workout_duration(&workout);

        let total_steps = workout.workout.workouts.len();

        let current_step = workout
            .workout
            .workouts
            .pop_front()
            .expect("Workout does not contain any workout steps");

        let current_step_duration = calculate_step_duration(&current_step);

        info!("Next step {current_step:?}");

        let next_step = workout.workout.workouts.front().cloned();

        Ok(ZwoWorkout {
            workout_file: workout,
            pending: None,
            workout_state: WorkoutState {
                total_steps,
                total_workout_duration,
                current_step_duration,
                current_step_idx: 1,
                current_step,
                next_step,
                current_power_set: 0,
                ftp_base,
            },
        })
    }

    pub fn pause(&mut self) {
        info!("Workout paused");
        let pending = self.pending.take();
        if let Some(timer) = pending {
            timer.abort();
        };
    }

    fn advance_workout(&mut self) -> Option<PowerDuration> {
        let next_pd = {
            if let Some(next_step) = self.workout_state.current_step.advance() {
                Some(next_step)
            } else {
                // Current step exhausted, get next one
                if let Some(next) = self.workout_file.workout.workouts.pop_front() {
                    // Start with next workout
                    self.set_current_step(next);

                    let next_pd = self
                        .workout_state
                        .current_step
                        .advance()
                        .expect("Cannot advance fresh workout step");

                    Some(next_pd)
                } else {
                    // Nothing left
                    None
                }
            }
        };

        if let Some(power_duration) = &next_pd {
            self.workout_state.current_power_set = self.get_power(power_duration.power_level);
        }

        next_pd

    }

    /// Sets workout step that is currently executed, together with workout state update
    fn set_current_step(&mut self, next: WorkoutSteps) {
        self.workout_state.current_step = next;
        self.workout_state.current_step_duration =
            calculate_step_duration(&self.workout_state.current_step);
        self.workout_state.current_step_idx += 1;
        self.workout_state.next_step = self.workout_file.workout.workouts.front().cloned();
    }

    fn get_power(&self, power_level: f64) -> i16 {
        (self.workout_state.ftp_base * power_level).round() as i16
    }
}

/// Returns real time to spent on given workout step
fn calculate_step_duration(workout_step: &WorkoutSteps) -> Duration {
    let step_duration = {
        let d = match workout_step {
            WorkoutSteps::Warmup(x) => x.duration,
            WorkoutSteps::Ramp(x) => x.duration,
            WorkoutSteps::SteadyState(x) => x.duration,
            WorkoutSteps::Cooldown(x) => x.duration,
            WorkoutSteps::IntervalsT(x) => (x.on_duration + x.off_duration) * x.repeat,
            WorkoutSteps::FreeRide(x) => x.duration,
        };

        Duration::from_secs(d)
    };
    step_duration
}

/// Total time this workout will take
fn calculate_total_workout_duration(workout: &WorkoutFile) -> Duration {
    let total_workout_duration = {
        workout
            .workout
            .workouts
            .iter()
            .fold(Duration::from_secs(0), |acc, step| {
                acc + calculate_step_duration(step)
            })
    };
    total_workout_duration
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
        (0, Some(self.workout_file.workout.workouts.len()))
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
