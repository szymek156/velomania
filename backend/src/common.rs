use std::time::Duration;



pub fn duration_to_string(duration: &Duration) -> String {
    const HOUR_IN_SECONDS: u64 = 3600;
    const MINUTE_IN_SECONDS: u64 = 60;

    let secs = duration.as_secs();

    let hours = secs / HOUR_IN_SECONDS;
    let secs = secs % HOUR_IN_SECONDS;

    let mins = secs / MINUTE_IN_SECONDS;
    let secs = secs % MINUTE_IN_SECONDS;

    let res = format!("{secs}s");

    let res = if mins > 0 {
        format!("{mins}m {res}")
    } else {
        res
    };

    let res = if hours > 0 {
        format!("{hours}h {res}")
    } else {
        res
    };

    res
}

pub fn get_power(ftp_base: f64, power_level: f64) -> i16 {
    (ftp_base * power_level).round() as i16
}

