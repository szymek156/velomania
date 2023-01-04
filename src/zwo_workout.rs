use std::{path::Path, pin::Pin, task::Poll, time::Duration};

use anyhow::{Context, Result};
use futures::{Future, Stream};

use tokio::{
    io::AsyncReadExt,
    pin,
    task::JoinHandle,
    time::{self, Instant, Sleep},
};

use crate::{
    cli::UserCommands,
    common::get_power,
    workout_state::WorkoutState,
    zwo_workout_file::{IntervalsT, PowerDuration, WorkoutFile, WorkoutSteps},
};

pub struct ZwoWorkout {
    workout_file: WorkoutFile,
    pending: Pin<Box<Sleep>>,
    pub workout_state: WorkoutState,
    pub current_step: WorkoutSteps,
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

        let workout_state = WorkoutState::new(&workout, ftp_base);

        let current_step = workout
            .workout
            .steps
            .pop_front()
            .expect("Workout does not contain any workout steps");

        info!("Next step {current_step:?}");

        Ok(ZwoWorkout {
            workout_file: workout,
            pending: Box::pin(tokio::time::sleep(Duration::from_secs(0))),
            workout_state,
            current_step,
        })
    }

    pub fn pause(&mut self) {
        info!("Workout paused");
        self.pending.as_mut().reset(Instant::now() + Duration::MAX)
        // let pending = self.pending.take();
        // if let Some(timer) = pending {
        //     timer.abort();
        // };
    }

    pub fn skip_step(&mut self) {
        info!("Skipping step");
        self.current_step.skip();
        self.pending = Box::pin(tokio::time::sleep(Duration::from_secs(0)));
        self.workout_state.handle_skip_step();
    }

    fn advance_workout(&mut self) -> Option<PowerDuration> {
        let next_pd = {
            if let Some(next_pd) = self.advance_step() {
                Some(next_pd)
            } else {
                // Current step exhausted, get next one
                self.workout_state.handle_next_step(&self.workout_file);

                if let Some(next) = self.workout_file.workout.steps.pop_front() {
                    // Start with next workout
                    self.current_step = next;

                    let next_pd = self
                        .advance_step()
                        .expect("Cannot advance fresh workout step");

                    Some(next_pd)
                } else {
                    // Nothing left
                    None
                }
            }
        };

        if let Some(power_duration) = &next_pd {
            self.workout_state.current_power_set =
                get_power(self.workout_state.ftp_base, power_duration.power_level);
        }

        next_pd
    }

    fn advance_step(&mut self) -> Option<PowerDuration> {
        self.workout_state.handle_step_advance(&self.current_step);
        self.current_step.advance()
    }
}

impl Stream for ZwoWorkout {
    type Item = UserCommands;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match self.pending.as_mut().poll(cx) {
            Poll::Ready(_) => {
                debug!("Timer ready, advancing workout");

                match self.advance_workout() {
                    Some(PowerDuration {
                        duration,
                        power_level,
                    }) => {
                        self.pending = Box::pin(tokio::time::sleep(duration));

                        Poll::Ready(Some(UserCommands::SetTargetPower {
                            power: get_power(self.workout_state.ftp_base, power_level),
                        }))
                    }

                    // Whole workout exhausted
                    None => Poll::Ready(None),
                }
            }
            // Previous step should be still executed
            Poll::Pending => Poll::Pending,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.workout_file.workout.steps.len()))
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
