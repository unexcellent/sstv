pub struct Frequency {
    hertz: u16,
}
impl Frequency {
    pub const fn from_hz(hertz: u16) -> Self {
        Self { hertz }
    }
    pub const fn hz(self) -> u16 {
        self.hertz
    }
}

#[macro_export]
macro_rules! Hz {
    ($value:expr) => {
        Frequency::from_hz($value)
    };
}

pub struct Duration {
    milliseconds: u16,
}
impl Duration {
    pub const fn from_ms(milliseconds: u16) -> Self {
        Self { milliseconds }
    }
    pub const fn ms(self) -> u16 {
        self.milliseconds
    }
}

#[macro_export]
macro_rules! ms {
    ($value:expr) => {
        Duration::from_ms($value)
    };
}
