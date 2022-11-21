use std::{fmt::Display, io, num::TryFromIntError, str::Utf8Error, string::FromUtf8Error};

use serde::{de, ser};

use crate::data::Type;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Expected(Type),
    DataError(String),
    EndOfFile,
    IO(io::Error),
    Nested(Box<dyn std::error::Error>),
    Msg(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Expected(t) => f.write_fmt(format_args!("Expected {}", t)),
            Error::DataError(t) => f.write_fmt(format_args!("Data Error: {}", t)),
            Error::EndOfFile => f.write_str("End Of File"),
            Error::IO(err) => err.fmt(f),
            Error::Nested(err) => err.fmt(f),
            Error::Msg(msg) => f.write_str(msg),
        }
    }
}

impl Error {
    #[inline]
    pub fn data_error(s: impl Into<String>) -> Self {
        Self::DataError(s.into())
    }

    #[inline]
    pub fn from_error<E: std::error::Error + 'static>(err: E) -> Self {
        Self::Nested(Box::new(err))
    }

    #[inline]
    pub fn from_box(err: Box<dyn std::error::Error>) -> Self {
        Self::Nested(err)
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
        Self::Msg(format!("{}", msg))
    }
}
impl de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        Self::Msg(format!("{}", msg))
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::IO(err)
    }
}
impl From<Utf8Error> for Error {
    fn from(err: Utf8Error) -> Self {
        Self::DataError(format!("{}", err))
    }
}
impl From<FromUtf8Error> for Error {
    fn from(err: FromUtf8Error) -> Self {
        Self::DataError(format!("{}", err))
    }
}
impl From<TryFromIntError> for Error {
    fn from(err: TryFromIntError) -> Self {
        Self::DataError(format!("{}", err))
    }
}
