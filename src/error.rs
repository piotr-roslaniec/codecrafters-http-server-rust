use std::io::Error as IoError;
use std::net::AddrParseError;

#[derive(Debug)]
pub enum ServerError {
    IO(IoError),
    AddrParse(AddrParseError),
    HttpError(HttpError),
}

#[derive(Debug)]
pub enum HttpError {
    MissingMethod,
    MissingPath,
    MissingVersion,
}

impl From<IoError> for ServerError {
    fn from(error: IoError) -> Self {
        ServerError::IO(error)
    }
}

impl From<AddrParseError> for ServerError {
    fn from(error: AddrParseError) -> Self {
        ServerError::AddrParse(error)
    }
}

pub type Result<T> = std::result::Result<T, ServerError>;
