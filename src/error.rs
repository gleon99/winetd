use config;
use log::SetLoggerError;
use snafu::Snafu;
use std::io;
use winapi::um::errhandlingapi;
use winapi::um::winsock2;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum Error {
    #[snafu(display("Couldn't initialize log: {}", error))]
    LogInitError { error: SetLoggerError },

    #[snafu(display("Invalid configuration file"))]
    ConfigParseError { path: String },

    #[snafu(display("Config error"))]
    ConfigError,

    #[snafu(display("I/O error: {}", error))]
    Io { error: io::Error },

    #[snafu(display("OS error: {}", unsafe {errhandlingapi::GetLastError()}))]
    OSError,

    #[snafu(display("WSAStartup error: {}", error))]
    WSAStartupError { error: i32 },

    #[snafu(display("WSA error: {}", unsafe {winsock2::WSAGetLastError()}))]
    WSAError,

    #[snafu(display("Environment Error"))]
    EnvError,

    #[snafu(display("Unknown I/O event while polling"))]
    UnknownEvent,
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::Io { error }
    }
}

impl From<SetLoggerError> for Error {
    fn from(error: SetLoggerError) -> Self {
        Error::LogInitError { error }
    }
}

impl From<config::ConfigError> for Error {
    fn from(error: config::ConfigError) -> Self {
        match error {
            config::ConfigError::FileParse { uri, .. } => {
                Error::ConfigParseError { path: uri.unwrap() }
            }
            _ => Error::ConfigError,
        }
    }
}
