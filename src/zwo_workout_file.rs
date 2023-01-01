use std::{collections::VecDeque, time::Duration};

use serde::{Deserialize, Serialize};

// XML schema definition
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkoutFile {
    pub author: String,
    pub name: String,
    pub description: String,
    pub sport_type: String,
    pub workout: Workout,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Workout {
    #[serde(rename = "$value")]
    pub steps: VecDeque<WorkoutSteps>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum WorkoutSteps {
    Warmup(Warmup),
    Ramp(Ramp),
    SteadyState(SteadyState),
    Cooldown(Cooldown),
    IntervalsT(IntervalsT),
    FreeRide(FreeRide),
}

pub(crate) trait WorkoutStep {
    fn advance(&mut self) -> Option<PowerDuration>;
}

impl WorkoutSteps {
    pub(crate) fn advance(&mut self) -> Option<PowerDuration> {
        match self {
            WorkoutSteps::Warmup(w) => w.advance(),
            WorkoutSteps::SteadyState(w) => w.advance(),
            WorkoutSteps::Cooldown(w) => w.advance(),
            WorkoutSteps::IntervalsT(w) => w.advance(),
            WorkoutSteps::Ramp(w) => w.advance(),
            WorkoutSteps::FreeRide(w) => w.advance(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct Warmup {
    pub duration: u64,
    pub power_low: f64,
    pub power_high: f64,
}

impl WorkoutStep for Warmup {
    /// Get power level lasting for one second from span [low; high)
    fn advance(&mut self) -> Option<PowerDuration> {
        if self.duration == 0 {
            return None;
        }

        let power_level = self.power_low;

        let span = self.power_high - self.power_low;
        let step = span / self.duration as f64;

        self.duration -= 1;
        self.power_low += step;
        Some(PowerDuration {
            duration: Duration::from_secs(1),
            power_level,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct Ramp {
    pub duration: u64,
    pub power_low: f64,
    pub power_high: f64,
}

impl WorkoutStep for Ramp {
    /// Get power level lasting for one second from span [low; high)
    fn advance(&mut self) -> Option<PowerDuration> {
        if self.duration == 0 {
            return None;
        }

        let power_level = self.power_low;

        let span = self.power_high - self.power_low;
        let step = span / self.duration as f64;

        self.duration -= 1;
        self.power_low += step;
        Some(PowerDuration {
            duration: Duration::from_secs(1),
            power_level,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct Cooldown {
    pub duration: u64,
    pub power_low: f64,
    pub power_high: f64,
}

impl WorkoutStep for Cooldown {
    /// Get power level lasting for one second from span [high; low)
    fn advance(&mut self) -> Option<PowerDuration> {
        if self.duration == 0 {
            return None;
        }

        let power_level = self.power_low;

        // In cool down, low keeps high value, high keeps low....
        let span = self.power_low - self.power_high;
        let step = span / self.duration as f64;

        self.duration -= 1;
        self.power_low -= step;
        Some(PowerDuration {
            duration: Duration::from_secs(1),
            power_level,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct SteadyState {
    pub duration: u64,
    pub power: f64,
}

impl WorkoutStep for SteadyState {
    fn advance(&mut self) -> Option<PowerDuration> {
        if self.duration == 0 {
            return None;
        }

        let duration = Duration::from_secs(self.duration);

        self.duration = 0;

        Some(PowerDuration {
            duration,
            power_level: self.power,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct IntervalsT {
    pub repeat: u64,
    pub on_duration: u64,
    pub off_duration: u64,
    pub on_power: f64,
    pub off_power: f64,

    #[serde(skip)]
    pub current_interval: usize,
}

impl WorkoutStep for IntervalsT {
    fn advance(&mut self) -> Option<PowerDuration> {
        if self.repeat == 0 {
            return None;
        }

        let step = if self.current_interval % 2 == 0 {
            Some(PowerDuration {
                duration: Duration::from_secs(self.on_duration),
                power_level: self.on_power,
            })
        } else {
            self.repeat -= 1;
            Some(PowerDuration {
                duration: Duration::from_secs(self.off_duration),
                power_level: self.off_power,
            })
        };

        self.current_interval += 1;

        step
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct FreeRide {
    pub duration: u64,
    pub flat_road: f64,
}

impl WorkoutStep for FreeRide {
    fn advance(&mut self) -> Option<PowerDuration> {
        if self.duration == 0 {
            return None;
        }

        let duration = Duration::from_secs(self.duration);

        self.duration = 0;

        Some(PowerDuration {
            duration,
            // TODO: there should be something like ERG mode off, IDK if 0 is valid
            power_level: 0.0,
        })
    }
}

/// How much power should be set for how long
#[derive(Debug, PartialEq, Clone)]
pub struct PowerDuration {
    pub duration: Duration,
    pub power_level: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warmup_works() {
        // Of course implementation suffers because of the rounding errors
        let mut w = Warmup {
            duration: 4,
            power_low: 0.0,
            power_high: 100.0,
        };

        assert_eq!(
            w.advance(),
            Some(PowerDuration {
                duration: Duration::from_secs(1),
                power_level: 0.0
            })
        );
        assert_eq!(
            w.advance(),
            Some(PowerDuration {
                duration: Duration::from_secs(1),
                power_level: 25.0
            })
        );
        assert_eq!(
            w.advance(),
            Some(PowerDuration {
                duration: Duration::from_secs(1),
                power_level: 50.0
            })
        );
        assert_eq!(
            w.advance(),
            Some(PowerDuration {
                duration: Duration::from_secs(1),
                power_level: 75.0
            })
        );
        // note no power level 100, that's the result of quantization.
        // in real workouts difference between last level end expected will not be that huge
        assert_eq!(w.advance(), None);
    }

    #[test]
    fn ramp_works() {
        // Of course implementation suffers because of the rounding errors
        let mut w = Ramp {
            duration: 4,
            power_low: 0.0,
            power_high: 100.0,
        };

        assert_eq!(
            w.advance(),
            Some(PowerDuration {
                duration: Duration::from_secs(1),
                power_level: 0.0
            })
        );
        assert_eq!(
            w.advance(),
            Some(PowerDuration {
                duration: Duration::from_secs(1),
                power_level: 25.0
            })
        );
        assert_eq!(
            w.advance(),
            Some(PowerDuration {
                duration: Duration::from_secs(1),
                power_level: 50.0
            })
        );
        assert_eq!(
            w.advance(),
            Some(PowerDuration {
                duration: Duration::from_secs(1),
                power_level: 75.0
            })
        );
        // note no power level 100, that's the result of quantization.
        // in real workouts difference between last level end expected will not be that huge
        assert_eq!(w.advance(), None);
    }

    #[test]
    fn cooldown_works() {
        // Of course implementation suffers because of the rounding errors
        let mut w = Cooldown {
            duration: 4,
            power_low: 100.0,
            power_high: 0.0,
        };

        assert_eq!(
            w.advance(),
            Some(PowerDuration {
                duration: Duration::from_secs(1),
                power_level: 100.0
            })
        );
        assert_eq!(
            w.advance(),
            Some(PowerDuration {
                duration: Duration::from_secs(1),
                power_level: 75.0
            })
        );
        assert_eq!(
            w.advance(),
            Some(PowerDuration {
                duration: Duration::from_secs(1),
                power_level: 50.0
            })
        );
        assert_eq!(
            w.advance(),
            Some(PowerDuration {
                duration: Duration::from_secs(1),
                power_level: 25.0
            })
        );
        // note no power level 0, that's the result of quantization.
        // in real workouts difference between last level end expected will not be that huge
        assert_eq!(w.advance(), None);
    }

    #[test]
    fn steady_works() {
        // Of course implementation suffers because of the rounding errors
        let mut w = SteadyState {
            duration: 4,
            power: 1.23,
        };

        assert_eq!(
            w.advance(),
            Some(PowerDuration {
                duration: Duration::from_secs(4),
                power_level: 1.23
            })
        );
        assert_eq!(w.advance(), None);
    }

    #[test]
    fn free_ride_works() {
        // Of course implementation suffers because of the rounding errors
        let mut w = FreeRide {
            duration: 4,
            flat_road: 1.0,
        };

        assert_eq!(
            w.advance(),
            Some(PowerDuration {
                duration: Duration::from_secs(4),
                power_level: 0.0
            })
        );
        assert_eq!(w.advance(), None);
    }

    #[test]
    fn intervals_t_works() {
        // Of course implementation suffers because of the rounding errors
        let mut w = IntervalsT {
            repeat: 3,
            on_duration: 10,
            off_duration: 20,
            on_power: 80.0,
            off_power: 150.0,
            current_interval: 0,
        };

        assert_eq!(
            w.advance(),
            Some(PowerDuration {
                duration: Duration::from_secs(10),
                power_level: 80.0
            })
        );

        assert_eq!(
            w.advance(),
            Some(PowerDuration {
                duration: Duration::from_secs(20),
                power_level: 150.0
            })
        );

        assert_eq!(
            w.advance(),
            Some(PowerDuration {
                duration: Duration::from_secs(10),
                power_level: 80.0
            })
        );

        assert_eq!(
            w.advance(),
            Some(PowerDuration {
                duration: Duration::from_secs(20),
                power_level: 150.0
            })
        );

        assert_eq!(
            w.advance(),
            Some(PowerDuration {
                duration: Duration::from_secs(10),
                power_level: 80.0
            })
        );

        assert_eq!(
            w.advance(),
            Some(PowerDuration {
                duration: Duration::from_secs(20),
                power_level: 150.0
            })
        );

        assert_eq!(w.advance(), None);
    }
}
