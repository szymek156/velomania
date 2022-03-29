use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[allow(non_snake_case)]
pub struct Warmup {
    Duration: usize,
    PowerLow: f64,
    PowerHigh: f64,
}

impl WorkoutStep for Warmup {
    /// Get power level lasting for one second from span [low; high)
    fn advance(&mut self) -> Option<PowerDuration> {
        if self.Duration == 0 {
            return None;
        }

        let power_level = self.PowerLow;

        let span = self.PowerHigh - self.PowerLow;
        let step = span / self.Duration as f64;

        self.Duration -= 1;
        self.PowerLow += step;
        Some(PowerDuration {
            duration: Duration::from_secs(1),
            power_level,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[allow(non_snake_case)]
pub struct Ramp {
    Duration: usize,
    PowerLow: f64,
    PowerHigh: f64,
}

impl WorkoutStep for Ramp {
    /// Get power level lasting for one second from span [low; high)
    fn advance(&mut self) -> Option<PowerDuration> {
        if self.Duration == 0 {
            return None;
        }

        let power_level = self.PowerLow;

        let span = self.PowerHigh - self.PowerLow;
        let step = span / self.Duration as f64;

        self.Duration -= 1;
        self.PowerLow += step;
        Some(PowerDuration {
            duration: Duration::from_secs(1),
            power_level,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[allow(non_snake_case)]
pub struct Cooldown {
    Duration: usize,
    PowerLow: f64,
    PowerHigh: f64,
}

impl WorkoutStep for Cooldown {
    /// Get power level lasting for one second from span [high; low)
    fn advance(&mut self) -> Option<PowerDuration> {
        if self.Duration == 0 {
            return None;
        }

        let power_level = self.PowerLow;

        // In cool down, low keeps high value, high keeps low....
        let span = self.PowerLow - self.PowerHigh;
        let step = span / self.Duration as f64;

        self.Duration -= 1;
        self.PowerLow -= step;
        Some(PowerDuration {
            duration: Duration::from_secs(1),
            power_level,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[allow(non_snake_case)]
pub struct SteadyState {
    Duration: u64,
    Power: f64,
}

impl WorkoutStep for SteadyState {
    fn advance(&mut self) -> Option<PowerDuration> {
        if self.Duration == 0 {
            return None;
        }

        let duration = Duration::from_secs(self.Duration);

        self.Duration = 0;

        Some(PowerDuration {
            duration,
            power_level: self.Power,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[allow(non_snake_case)]
pub struct IntervalsT {
    Repeat: usize,
    OnDuration: u64,
    OffDuration: u64,
    OnPower: f64,
    OffPower: f64,

    #[serde(skip)]
    current_step: usize,
}

impl WorkoutStep for IntervalsT {
    fn advance(&mut self) -> Option<PowerDuration> {
        if self.Repeat == 0 {
            return None;
        }

        let step = if self.current_step % 2 == 0 {
            Some(PowerDuration {
                duration: Duration::from_secs(self.OnDuration),
                power_level: self.OnPower,
            })
        } else {
            self.Repeat -= 1;
            Some(PowerDuration {
                duration: Duration::from_secs(self.OffDuration),
                power_level: self.OffPower,
            })
        };

        self.current_step += 1;

        step
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[allow(non_snake_case)]
pub struct FreeRide {
    Duration: u64,
    FlatRoad: f64,
}

impl WorkoutStep for FreeRide {
    fn advance(&mut self) -> Option<PowerDuration> {
        if self.Duration == 0 {
            return None;
        }

        let duration = Duration::from_secs(self.Duration);

        self.Duration = 0;

        Some(PowerDuration {
            duration,
            // TODO: there should be something like ERG mode off, IDK if 0 is valid
            power_level: 0.0,
        })
    }
}

/// How much power should be set for how long
#[derive(Debug, PartialEq)]
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
            Duration: 4,
            PowerLow: 0.0,
            PowerHigh: 100.0,
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
            Duration: 4,
            PowerLow: 0.0,
            PowerHigh: 100.0,
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
            Duration: 4,
            PowerLow: 100.0,
            PowerHigh: 0.0,
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
            Duration: 4,
            Power: 1.23,
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
            Duration: 4,
            FlatRoad: 1.0,
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
            Repeat: 3,
            OnDuration: 10,
            OffDuration: 20,
            OnPower: 80.0,
            OffPower: 150.0,
            current_step: 0,
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
