use std::{
    cell::BorrowMutError,
    io::{self, ErrorKind},
};

use enum_as_inner::EnumAsInner;
use thiserror::Error;

pub type MbonResult<T> = Result<T, MbonError>;

#[derive(Debug, Error, EnumAsInner)]
pub enum MbonError {
    #[error("Expected more data to parse")]
    OutOfData,
    #[error("Invalid mark")]
    InvalidMark,
    #[error("Invalid Signature")]
    InvalidSignature,
    #[error("Invalid Data: {0}")]
    InvalidData(anyhow::Error),
    #[error("Internal Error: {0}")]
    InternalError(String),
    #[error("{0}")]
    IOError(io::Error),
}

impl From<io::Error> for MbonError {
    fn from(err: io::Error) -> Self {
        match err.kind() {
            ErrorKind::UnexpectedEof => Self::OutOfData,
            _ => Self::IOError(err),
        }
    }
}

impl From<BorrowMutError> for MbonError {
    fn from(value: BorrowMutError) -> Self {
        Self::InternalError(value.to_string())
    }
}
