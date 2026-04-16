use core::fmt;

#[derive(Debug)]
pub enum Error {
    EmptyImage,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyImage => write!(
                f,
                "The supplied image is empty. Was the pixel iterator already used?"
            ),
        }
    }
}

impl core::error::Error for Error {}

pub type Result<T> = core::result::Result<T, Error>;
