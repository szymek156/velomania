use std::{time::Duration};


use serde::Serialize;
use tokio::time::Instant;

use crate::{
    zwo_workout_file::{WorkoutFile, WorkoutSteps},
};

#[derive(Debug, Clone, Serialize)]
pub struct StepState {
    pub duration: Duration,
    pub step: WorkoutSteps,
    pub elapsed: Duration,
    #[serde(skip)]
    started: Instant,
}

#[derive(Debug, Clone, Serialize)]
pub struct IntervalState {
    pub repetition: usize,
    pub is_work_interval: bool,
    pub elapsed: Duration,
    pub duration: Duration,
    #[serde(skip)]
    started: Instant,
}

#[derive(Debug, Clone, Serialize)]
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
    #[serde(skip)]
    workout_started: Instant,
}



impl WorkoutState {

    pub(crate) fn new(workout: &WorkoutFile, ftp_base: f64) -> Self {
        let total_workout_duration = workout.total_workout_duration;

        let total_steps = workout.workout.steps.len();

        let current_workout_step = workout
            .workout
            .steps
            .get(0)
            .expect("Workout does not contain any steps")
            .clone();

        let current_step = StepState {
            duration: current_workout_step.get_step_duration(),
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

            self.current_step.duration = self.current_step.step.get_step_duration();
            self.current_step_number += 1;

            self.current_step.elapsed = Duration::from_secs(0);
            self.current_step.started = Instant::now();

            // Clear interval info if step is not interval
            match self.current_step.step {
                WorkoutSteps::IntervalsT(_) => (),
                _ => self.current_interval = None,
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

    pub(crate) fn handle_skip_step(&mut self) {
        let remaining_time = {
            if let Some(interval) = &self.current_interval {
                interval.duration.saturating_sub(interval.elapsed)
            } else {
                self.current_step
                    .duration
                    .saturating_sub(self.current_step.elapsed)
            }
        };
        self.total_workout_duration = self.total_workout_duration.saturating_sub(remaining_time);
    }
}
