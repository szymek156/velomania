use std::{fmt::Display, task::Poll, time::Duration};

use futures::Stream;
use tokio::time::Instant;

use crate::{
    common::get_power,
    zwo_workout_file::{WorkoutFile, WorkoutSteps},
};

#[derive(Debug, Clone)]
pub struct WorkoutState {
    pub total_steps: usize,

    pub total_workout_duration: Duration,
    pub current_step_duration: Duration,

    pub current_step_number: usize,
    pub current_step: WorkoutSteps,
    pub next_step: Option<WorkoutSteps>,

    pub current_power_set: i16,
    pub ftp_base: f64,

    pub workout_elapsed: Duration,
    pub step_elapsed: Duration,

    workout_started: Instant,
    step_started: Instant,
}

impl WorkoutState {
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
                .steps
                .iter()
                .fold(Duration::from_secs(0), |acc, step| {
                    acc + Self::calculate_step_duration(step)
                })
        };
        total_workout_duration
    }

    /// Sets workout step that is currently executed, together with workout state update
    pub fn update_state(&mut self, workout: &WorkoutFile) {
        if let Some(next) = workout.workout.steps.front() {
            self.current_step = next.clone();

            self.current_step_duration = Self::calculate_step_duration(&self.current_step);
            self.current_step_number += 1;

            self.step_elapsed = Duration::from_secs(0);
            self.step_started = Instant::now();

            self.next_step = workout.workout.steps.get(1).cloned();
        }
    }

    pub fn update_ts(&mut self) {
        let instant = Instant::now();
        self.step_elapsed = instant - self.step_started;
        self.workout_elapsed = instant - self.workout_started;
    }

    pub(crate) fn new(workout: &WorkoutFile, ftp_base: f64) -> Self {
        let total_workout_duration = Self::calculate_total_workout_duration(&workout);

        let total_steps = workout.workout.steps.len();

        let current_step = workout
            .workout
            .steps
            .get(0)
            .expect("Workout does not contain any steps")
            .clone();
        let current_step_duration = Self::calculate_step_duration(&current_step);
        let next_step = workout.workout.steps.get(1).cloned();

        Self {
            total_steps,
            total_workout_duration,
            current_step_duration,
            // Note it's 1-based for human readability!
            current_step_number: 1,
            current_step,
            next_step,
            current_power_set: 0,
            ftp_base,
            workout_elapsed: Duration::from_secs(0),
            step_elapsed: Duration::from_secs(0),
            workout_started: Instant::now(),
            step_started: Instant::now(),
        }
    }
}
