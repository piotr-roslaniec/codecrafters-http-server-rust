use std::{io::Error as IoError, net::AddrParseError};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("IO error: {0}")]
    IO(#[from] IoError),
    #[error("Address parse error: {0}")]
    AddrParse(#[from] AddrParseError),
    #[error("HTTP error: {0}")]
    HttpError(#[from] HttpError),
}

#[derive(Error, Debug)]
pub enum HttpError {
    #[error("Missing method")]
    MissingMethod,
    #[error("Missing path")]
    MissingPath,
    #[error("Missing version")]
    MissingVersion,
    #[error("Missing header key")]
    MissingHeaderKey,
    #[error("Missing header value")]
    MissingHeaderValue,
    #[error("Invalid content length")]
    InvalidContentLength,
    #[error("Missing request line")]
    MissingRequestLine,
    #[error("Empty request line")]
    EmptyRequestLine,
    #[error("Unsupported version")]
    UnsupportedVersion,
}
