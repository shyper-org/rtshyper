// Reference: https://crates.io/crates/ta
// Here, in an OS, only General Purpose Registers are avaliable.
// So, we only use integer to calculate the EMA

type Result<T> = core::result::Result<T, TaError>;

#[allow(dead_code)]
#[derive(Debug)]
pub enum TaError {
    InvalidParameter,
    DataItemIncomplete,
    DataItemInvalid,
}

#[derive(Debug)]
pub struct ExponentialMovingAverage {
    period: usize,
    k: f32,
    current: f32,
    is_new: bool,
}

impl ExponentialMovingAverage {
    pub fn new(period: usize) -> Result<Self> {
        match period {
            0 => Err(TaError::InvalidParameter),
            _ => Ok(Self {
                period,
                k: 2.0 / (period + 1) as f32,
                current: 0.0,
                is_new: true,
            }),
        }
    }

    // EMA_t = alpha * p_t + (1 - alpha) * EMA_(t-1)
    pub fn next(&mut self, input: f32) -> f32 {
        self.current = if self.is_new {
            self.is_new = false;
            input
        } else {
            self.k * input + (1.0 - self.k) * self.current
        };
        self.current
    }

    #[allow(dead_code)]
    pub fn period(&self) -> usize {
        self.period
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.current = 0.0;
        self.is_new = true;
    }
}

impl Default for ExponentialMovingAverage {
    fn default() -> Self {
        Self::new(9).unwrap()
    }
}
