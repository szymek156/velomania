use std::marker::PhantomData;

/// From GATT_Specification_Supplement_v5
/// Converts raw bytes to scalar type
#[derive(Debug)]
pub struct ScalarType<T> {
    multiplier: i32, // valid range is -10, 10
    base_10: f64,    // 10^d
    base_2: f64,     // 2^b
    marker: PhantomData<T>,
}

impl<T> ScalarType<T>
where
    T: Into<f64>,
{
    pub fn new() -> Self {
        // Default values
        Self {
            multiplier: 1, // M = 1
            base_10: 1.0,  // d = 0
            base_2: 1.0,   // b = 0
            marker: PhantomData,
        }
    }

    pub fn with_multiplier(mut self, multiplier: i32) -> Self {
        self.multiplier = multiplier;

        self
    }

    pub fn with_dec_exp(mut self, dec_exp: i32) -> Self {
        self.base_10 = 10.0f64.powi(dec_exp);

        self
    }

    pub fn with_bin_exp(mut self, bin_exp: i32) -> Self {
        self.base_2 = 2.0f64.powi(bin_exp);

        self
    }

    // TODO: It's probably possible to use From/Into trait magic
    pub fn to_scalar(&self, raw: T) -> f64 {
        let raw: f64 = raw.into();

        raw * self.multiplier as f64 * self.base_10 * self.base_2
    }
}
