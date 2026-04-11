#[derive(Debug, Clone, Copy, PartialEq)]
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

#[macro_export]
macro_rules! Hz {
    ($value:expr) => {
        $crate::units::Frequency::from_hz($value)
    };
}

#[derive(Debug, Clone, Copy, PartialEq)]
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
