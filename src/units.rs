use core::ops::{Add, Div, Mul, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Frequency {
    hertz: u32,
}

impl Frequency {
    pub const fn from_hz(hertz: u32) -> Self {
        Self { hertz }
    }

    pub const fn hz(self) -> u32 {
        self.hertz
    }
}

impl Add for Frequency {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self::from_hz(self.hz() + rhs.hz())
    }
}

impl Sub for Frequency {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self::from_hz(self.hz() - rhs.hz())
    }
}

impl Mul<u32> for Frequency {
    type Output = Self;

    fn mul(self, rhs: u32) -> Self {
        Self::from_hz(self.hz() * rhs)
    }
}

impl Div<u32> for Frequency {
    type Output = Self;

    fn div(self, rhs: u32) -> Self {
        Self::from_hz(self.hz() / rhs)
    }
}

#[macro_export]
macro_rules! Hz {
    ($value:expr) => {
        $crate::units::Frequency::from_hz($value)
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Duration {
    microseconds: u32,
}

impl Duration {
    pub const fn from_us(microseconds: u32) -> Self {
        Self { microseconds }
    }

    pub const fn from_ms(milliseconds: u32) -> Self {
        Self {
            microseconds: milliseconds * 1000,
        }
    }

    pub const fn micros(self) -> u32 {
        self.microseconds
    }
}

impl Add for Duration {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self::from_us(self.micros() + rhs.micros())
    }
}

impl Sub for Duration {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self::from_us(self.micros() - rhs.micros())
    }
}

impl Mul<u32> for Duration {
    type Output = Self;

    fn mul(self, rhs: u32) -> Self {
        Self::from_us(self.micros() * rhs)
    }
}

impl Div<u32> for Duration {
    type Output = Self;

    fn div(self, rhs: u32) -> Self {
        Self::from_us(self.micros() / rhs)
    }
}

#[macro_export]
macro_rules! ms {
    ($value:expr) => {
        $crate::units::Duration::from_ms($value)
    };
}

#[macro_export]
macro_rules! us {
    ($value:expr) => {
        $crate::units::Duration::from_us($value)
    };
}
