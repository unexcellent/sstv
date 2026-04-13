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
    nanoseconds: u64,
}

impl Duration {
    pub const fn from_ns(nanoseconds: u64) -> Self {
        Self { nanoseconds }
    }

    pub const fn from_us(microseconds: u64) -> Self {
        Self {
            nanoseconds: microseconds * 1000,
        }
    }

    pub const fn from_ms(milliseconds: u64) -> Self {
        Self {
            nanoseconds: milliseconds * 1_000_000,
        }
    }

    pub const fn nanos(self) -> u64 {
        self.nanoseconds
    }

    pub const fn micros(self) -> u32 {
        (self.nanoseconds / 1000) as u32
    }
}

impl Add for Duration {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self::from_ns(self.nanos() + rhs.nanos())
    }
}

impl Sub for Duration {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self::from_ns(self.nanos() - rhs.nanos())
    }
}

impl Mul<u32> for Duration {
    type Output = Self;

    fn mul(self, rhs: u32) -> Self {
        Self::from_ns(self.nanos() * (rhs as u64))
    }
}

impl Div<u32> for Duration {
    type Output = Self;

    fn div(self, rhs: u32) -> Self {
        Self::from_ns(self.nanos() / (rhs as u64))
    }
}

#[macro_export]
macro_rules! ns {
    ($value:expr) => {
        $crate::units::Duration::from_ns($value)
    };
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
