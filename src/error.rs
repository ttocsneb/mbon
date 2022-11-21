use std::{fmt::Display, io, num::TryFromIntError, str::Utf8Error, string::FromUtf8Error};

use serde::{de, ser};

use crate::data::Type;

pub type Result<T> = std::result::Result<T, Error>;

/// The base error type for mbon
#[derive(Debug)]
pub enum Error {
    /// A type was expected, but a different one was found
    Expected(Type),
    /// There was a problem with the provided data
    DataError(String),
    /// There is no more data left, but more was expected
    EndOfFile,
    /// There was a problem reading the data
    IO(io::Error),
    /// There was a problem on the user's end
    Msg(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Expected(t) => f.write_fmt(format_args!("Expected {}", t)),
            Error::DataError(t) => f.write_fmt(format_args!("Data Error: {}", t)),
            Error::EndOfFile => f.write_str("More data was expected"),
            Error::IO(err) => err.fmt(f),
            Error::Msg(msg) => f.write_str(msg),
        }
    }
}

impl Error {
    #[inline]
    pub fn data_error(s: impl Display) -> Self {
        Self::DataError(s.to_string())
    }

    #[inline]
    pub fn msg(s: impl Display) -> Self {
        Self::Msg(s.to_string())
    }

    #[inline]
    pub fn from_error<E: std::error::Error + 'static>(err: E) -> Self {
        Self::msg(err)
    }

    #[inline]
    pub fn from_box(err: Box<dyn std::error::Error>) -> Self {
        Self::msg(err)
    }

    pub fn from_res<T, E>(res: std::result::Result<T, E>) -> Result<T>
    where
        E: std::error::Error + 'static,
    {
        match res {
            Ok(t) => Ok(t),
            Err(e) => Err(Self::from_error(e)),
        }
    }

    pub fn from_box_res<T>(res: std::result::Result<T, Box<dyn std::error::Error>>) -> Result<T> {
        match res {
            Ok(t) => Ok(t),
            Err(e) => Err(Self::from_box(e)),
        }
    }
}

impl std::error::Error for Error {}

impl ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        Self::msg(msg)
    }
}
impl de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        Self::msg(msg)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::IO(err)
    }
}
impl From<Utf8Error> for Error {
    fn from(err: Utf8Error) -> Self {
        Self::data_error(err)
    }
}
impl From<FromUtf8Error> for Error {
    fn from(err: FromUtf8Error) -> Self {
        Self::data_error(err)
    }
}
impl From<TryFromIntError> for Error {
    fn from(err: TryFromIntError) -> Self {
        Self::data_error(err)
    }
}
