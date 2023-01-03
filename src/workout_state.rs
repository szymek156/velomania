use std::{fmt::Display, task::Poll, time::Duration};

use futures::Stream;
use tokio::time::Instant;

use crate::{
    common::get_power,
    zwo_workout_file::{WorkoutFile, WorkoutSteps},
};

#[derive(Debug, Clone)]
pub struct StepState {
    pub duration: Duration,
    pub step: WorkoutSteps,
    pub elapsed: Duration,
    started: Instant,
}

#[derive(Debug, Clone)]
pub struct IntervalState {
    pub repetition: usize,
    pub is_work_interval: bool,
    pub elapsed: Duration,
    pub duration: Duration,
    started: Instant,
}

#[derive(Debug, Clone)]
pub struct WorkoutState {
    pub total_steps: usize,
    pub current_step_number: usize,

    pub total_workout_duration: Duration,

    pub next_step: Option<WorkoutSteps>,

    pub current_power_set: i16,
    pub ftp_base: f64,

    pub current_step: StepState,
    pub current_interval: Option<IntervalState>,
    pub workout_elapsed: Duration,
    workout_started: Instant,
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

    pub(crate) fn new(workout: &WorkoutFile, ftp_base: f64) -> Self {
        let total_workout_duration = Self::calculate_total_workout_duration(&workout);

        let total_steps = workout.workout.steps.len();

        let current_workout_step = workout
            .workout
            .steps
            .get(0)
            .expect("Workout does not contain any steps")
            .clone();

        let current_step = StepState {
            duration: Self::calculate_step_duration(&current_workout_step),
            step: current_workout_step,
            elapsed: Duration::from_secs(0),
            started: Instant::now(),
        };

        let next_step = workout.workout.steps.get(1).cloned();
        Self {
            total_steps,
            total_workout_duration,
            // Note it's 1-based for human readability!
            current_step_number: 1,
            current_step,
            next_step,
            current_interval: None,
            current_power_set: 0,
            ftp_base,
            workout_elapsed: Duration::from_secs(0),
            workout_started: Instant::now(),
        }
    }

    /// Sets workout step that is currently executed, together with workout state update
    pub fn handle_next_step(&mut self, workout: &WorkoutFile) {
        if let Some(next) = workout.workout.steps.front() {
            self.current_step.step = next.clone();

            self.current_step.duration = Self::calculate_step_duration(&self.current_step.step);
            self.current_step_number += 1;

            self.current_step.elapsed = Duration::from_secs(0);
            self.current_step.started = Instant::now();

            // Clear interval info if step is not interval
            match self.current_step.step {
                WorkoutSteps::IntervalsT(_) => (),
                _ => self.current_interval = None
            }

            self.next_step = workout.workout.steps.get(1).cloned();
        }
    }

    pub fn update_ts(&mut self) {
        let instant = Instant::now();
        self.current_step.elapsed = instant - self.current_step.started;
        self.workout_elapsed = instant - self.workout_started;

        if let Some(ref mut interval_state) = self.current_interval {
            interval_state.elapsed = instant - interval_state.started;
        }
    }

    pub(crate) fn handle_step_advance(&mut self, current_step: &WorkoutSteps) {
        if let WorkoutSteps::IntervalsT(interval) = current_step {
            let interval_duration = if interval.is_work_interval() {
                interval.on_duration
            } else {
                interval.off_duration
            };

            self.current_interval = Some(IntervalState {
                is_work_interval: interval.is_work_interval(),
                repetition: interval.current_interval / 2 + 1,
                elapsed: Duration::from_secs(0),
                duration: Duration::from_secs(interval_duration),
                started: Instant::now(),
            })
        }
    }
}
