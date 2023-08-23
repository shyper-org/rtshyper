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
    k: (usize, usize),
    current: usize,
    is_new: bool,
}

impl ExponentialMovingAverage {
    pub fn new(period: usize) -> Result<Self> {
        match period {
            0 => Err(TaError::InvalidParameter),
            _ => Ok(Self {
                period,
                k: (2, period + 1),
                current: 0,
                is_new: true,
            }),
        }
    }

    // EMA_t = alpha * p_t + (1 - alpha) * EMA_(t-1)
    pub fn next(&mut self, input: usize) -> usize {
        self.current = if self.is_new {
            self.is_new = false;
            input
        } else {
            (self.k.0 * input + (self.k.1 - self.k.0) * self.current) / self.k.1
        };
        self.current
    }

    #[allow(dead_code)]
    pub fn period(&self) -> usize {
        self.period
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.current = 0;
        self.is_new = true;
    }
}

impl Default for ExponentialMovingAverage {
    fn default() -> Self {
        Self::new(9).unwrap()
    }
}
